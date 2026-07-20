// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/engine.ts (OcrEngine.recognizePage/detect/recognizeLine)
//
//! ONNX Runtime engine — only compiled under the `ocr` feature (pulls `ort` + `image`).
//!
//! Targets `ort` 2.0.0-rc.x. The ONNX-facing surface here is small and isolated on
//! purpose: all numeric logic lives in the pure sibling modules (unit-tested without
//! models). If an `ort` version bump changes `Tensor::from_array` / `inputs!` /
//! `try_extract_tensor`, only this file needs touching.
//!
//! Robustness note: we derive the rec class count as `dict.len() + 2`
//! (blank + dict + space) instead of reading the output tensor shape, so the CTC
//! decode never depends on how `ort` reports dims.

use image::{imageops, imageops::FilterType, RgbaImage};
use std::sync::{Mutex, OnceLock};

use ort::session::Session;
use ort::value::Tensor;

use super::detect::{component_boxes, unclip_and_scale, DetBox, DET_MIN_SIZE};
use super::models::{self, OCR_DET_MODEL, OCR_REC_DICT, OCR_REC_MODEL};
use super::preprocess::{det_target_size, normalize_det, normalize_rec, rec_width, REC_HEIGHT};
use super::recognize::ctc_decode;
use super::{OcrError, OcrLine, Result, TEXT_SCORE};

impl From<ort::Error> for OcrError {
    fn from(e: ort::Error) -> Self {
        OcrError::Runtime(e.to_string())
    }
}

/// A loaded det + rec model pair with the recognition dictionary.
pub struct OcrEngine {
    det: Session,
    rec: Session,
    dict: Vec<String>,
}

impl OcrEngine {
    /// Load both models + dictionary from the cache. Errors if models are absent.
    pub fn load() -> Result<Self> {
        let dir = models::ocr_models_dir();
        if !models::models_present() {
            return Err(OcrError::ModelsMissing(dir.display().to_string()));
        }
        let yml = std::fs::read_to_string(dir.join(OCR_REC_DICT.filename))
            .map_err(|e| OcrError::Runtime(e.to_string()))?;
        let dict = models::parse_character_dict(&yml);
        if dict.is_empty() {
            return Err(OcrError::DictParse);
        }
        let det = Session::builder()?.commit_from_file(dir.join(OCR_DET_MODEL.filename))?;
        let rec = Session::builder()?.commit_from_file(dir.join(OCR_REC_MODEL.filename))?;
        Ok(Self { det, rec, dict })
    }

    /// Recognize all text lines in an RGBA image. Coordinates are input pixels.
    /// Takes `&mut self` because `ort::Session::run` needs a mutable borrow.
    pub fn recognize_image(&mut self, img: &RgbaImage, w: u32, h: u32) -> Result<Vec<OcrLine>> {
        if w < DET_MIN_SIZE as u32 || h < DET_MIN_SIZE as u32 {
            return Ok(Vec::new());
        }
        let boxes = self.detect(img, w, h)?;
        let mut lines = Vec::new();
        for b in boxes {
            if let Some(line) = self.recognize_line(img, b)? {
                if line.confidence >= TEXT_SCORE && !line.text.trim().is_empty() {
                    lines.push(line);
                }
            }
        }
        lines.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));
        Ok(lines)
    }

    /// Convenience: decode raw RGBA bytes (len = w*h*4).
    pub fn recognize_rgba(&mut self, rgba: &[u8], w: u32, h: u32) -> Result<Vec<OcrLine>> {
        let img = RgbaImage::from_raw(w, h, rgba.to_vec())
            .ok_or_else(|| OcrError::ImageDecode("rgba buffer size mismatch".into()))?;
        self.recognize_image(&img, w, h)
    }

    fn detect(&mut self, img: &RgbaImage, w: u32, h: u32) -> Result<Vec<DetBox>> {
        let (dw, dh) = det_target_size(w as usize, h as usize);
        let resized = imageops::resize(img, dw as u32, dh as u32, FilterType::Triangle);
        let rgb = rgba_to_rgb(&resized);
        let input = normalize_det(&rgb, dw, dh);
        let tensor = Tensor::from_array(([1usize, 3, dh, dw], input))?;
        let outputs = self.det.run(ort::inputs![tensor])?;
        let (_shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        if data.len() < dw * dh {
            return Ok(Vec::new());
        }
        let raw = component_boxes(&data[..dw * dh], dw, dh);
        Ok(unclip_and_scale(&raw, w, h, dw, dh))
    }

    fn recognize_line(&mut self, img: &RgbaImage, b: DetBox) -> Result<Option<OcrLine>> {
        let (rw, padded) = rec_width(b.w, b.h);
        let crop = imageops::crop_imm(img, b.x, b.y, b.w, b.h).to_image();
        let resized = imageops::resize(&crop, rw as u32, REC_HEIGHT as u32, FilterType::Triangle);
        let rgb = rgba_to_rgb(&resized);
        let input = normalize_rec(&rgb, rw, padded);
        let tensor = Tensor::from_array(([1usize, 3, REC_HEIGHT, padded], input))?;
        let outputs = self.rec.run(ort::inputs![tensor])?;
        let (_shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        // rec classes = blank(1) + dict + space(1); T inferred from length.
        let c = self.dict.len() + 2;
        if data.is_empty() || c == 0 || data.len() < c {
            return Ok(None);
        }
        let t = data.len() / c;
        Ok(ctc_decode(data, t, c, &self.dict).map(|(text, confidence)| OcrLine {
            text,
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
            confidence,
        }))
    }
}

/// Flatten RGBA → tightly packed RGB (drop alpha).
fn rgba_to_rgb(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width() * img.height() * 3) as usize);
    for px in img.pixels() {
        out.push(px[0]);
        out.push(px[1]);
        out.push(px[2]);
    }
    out
}

// Process-wide engine cache — model/session init is expensive; reuse across calls.
// Wrapped in a Mutex because `Session::run` needs `&mut self`.
static ENGINE: OnceLock<Mutex<OcrEngine>> = OnceLock::new();

fn engine() -> Result<&'static Mutex<OcrEngine>> {
    if let Some(e) = ENGINE.get() {
        return Ok(e);
    }
    // Load outside the cache first so a load failure is not cached (models may be
    // installed later). A benign race just discards the loser's engine.
    let loaded = OcrEngine::load()?;
    Ok(ENGINE.get_or_init(|| Mutex::new(loaded)))
}

/// Decode encoded image bytes (PNG/JPEG/…) and recognize text lines.
pub fn ocr_image_impl(bytes: &[u8]) -> Result<Vec<OcrLine>> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| OcrError::ImageDecode(e.to_string()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let mut guard = engine()?
        .lock()
        .map_err(|_| OcrError::Runtime("OCR engine mutex poisoned".into()))?;
    guard.recognize_image(&img, w, h)
}

#[cfg(test)]
mod smoke {
    use super::*;

    /// E2E: loads real models if present, else skips gracefully (CI has no models).
    #[test]
    fn engine_loads_and_runs_when_models_present() {
        if !models::models_present() {
            eprintln!("[ocr] models absent — skipping E2E smoke test");
            return;
        }
        let mut engine = OcrEngine::load().expect("engine load");
        let img = RgbaImage::from_pixel(96, 48, image::Rgba([255, 255, 255, 255]));
        // A blank white page must not panic; it should yield no confident lines.
        let lines = engine.recognize_image(&img, 96, 48).expect("recognize");
        assert!(lines.iter().all(|l| l.confidence >= TEXT_SCORE));
    }
}
