// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/engine.ts (detect/recognizeLine preprocessing)
//
//! Pure image preprocessing math for det/rec, plus target-size arithmetic.
//! No image-crate / ort dependency here so it compiles and unit-tests under
//! default features. Actual pixel resize/crop lives in [`super::engine`].

/// det long side (official `resize_long`).
pub const DET_LONG_SIDE: usize = 960;
/// rec fixed input height.
pub const REC_HEIGHT: usize = 48;
pub const REC_MIN_WIDTH: usize = 320;
pub const REC_MAX_WIDTH: usize = 3200;

// det: BGR channel order, mean/std applied in the yml-listed order (mean[0] → B).
pub const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
pub const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];

/// det resize target — long side scaled to 960, both dims rounded to a multiple
/// of 32 (min 32). Returns `(dw, dh)`.
pub fn det_target_size(w: usize, h: usize) -> (usize, usize) {
    let long = w.max(h).max(1) as f64;
    let ratio = DET_LONG_SIDE as f64 / long;
    let round32 = |v: f64| -> usize { (((v / 32.0).round()) as usize * 32).max(32) };
    (round32(w as f64 * ratio), round32(h as f64 * ratio))
}

/// HWC RGB (u8, len = w*h*3) → CHW BGR f32 (len = 3*w*h), det normalization.
/// `input[c*plane + i] = (channel/255 - mean[c]) / std[c]` with channel order B,G,R.
pub fn normalize_det(rgb: &[u8], w: usize, h: usize) -> Vec<f32> {
    let plane = w * h;
    let mut out = vec![0.0f32; 3 * plane];
    debug_assert!(rgb.len() >= plane * 3);
    for i in 0..plane {
        let r = rgb[i * 3] as f32 / 255.0;
        let g = rgb[i * 3 + 1] as f32 / 255.0;
        let b = rgb[i * 3 + 2] as f32 / 255.0;
        out[i] = (b - DET_MEAN[0]) / DET_STD[0];
        out[plane + i] = (g - DET_MEAN[1]) / DET_STD[1];
        out[2 * plane + i] = (r - DET_MEAN[2]) / DET_STD[2];
    }
    out
}

/// rec target width for a detected box: `rw` = aspect-preserving width at height 48
/// (clamped 16..=3200), `padded` = right-padded input width (>= 320).
pub fn rec_width(box_w: u32, box_h: u32) -> (usize, usize) {
    let bh = box_h.max(1) as f64;
    let raw = ((box_w as f64 * REC_HEIGHT as f64) / bh).round() as i64;
    let rw = raw.clamp(16, REC_MAX_WIDTH as i64) as usize;
    let padded = rw.max(REC_MIN_WIDTH);
    (rw, padded)
}

/// HWC RGB crop (u8, len = rw*REC_HEIGHT*3) → CHW BGR f32 (len = 3*padded*REC_HEIGHT),
/// rec normalization `(x/255 - 0.5)/0.5` = `x/255/0.5 - 1`, right zero-padded.
pub fn normalize_rec(rgb: &[u8], rw: usize, padded: usize) -> Vec<f32> {
    let plane = padded * REC_HEIGHT;
    let mut out = vec![0.0f32; 3 * plane];
    debug_assert!(rgb.len() >= rw * REC_HEIGHT * 3);
    let norm = |v: u8| -> f32 { (v as f32 / 255.0) / 0.5 - 1.0 };
    for y in 0..REC_HEIGHT {
        for x in 0..rw {
            let src = (y * rw + x) * 3;
            let dst = y * padded + x;
            out[dst] = norm(rgb[src + 2]); // B
            out[plane + dst] = norm(rgb[src + 1]); // G
            out[2 * plane + dst] = norm(rgb[src]); // R
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_size_is_multiple_of_32_and_long_side_960() {
        let (dw, dh) = det_target_size(1000, 500);
        assert_eq!(dw % 32, 0);
        assert_eq!(dh % 32, 0);
        // long side (width) maps to ~960
        assert!((dw as i64 - 960).abs() <= 32);
        assert!(dh <= dw);
    }

    #[test]
    fn target_size_floors_short_side_at_32() {
        // Very lopsided page: the short side rounds toward 0 and is floored to 32,
        // while the long side still targets 960 (reference resize_long behavior).
        let (dw, dh) = det_target_size(1000, 1);
        assert!((dw as i64 - 960).abs() <= 32);
        assert_eq!(dh, 32);
    }

    #[test]
    fn normalize_det_channel_mapping() {
        // single pixel, R=255 G=0 B=0
        let rgb = [255u8, 0, 0];
        let out = normalize_det(&rgb, 1, 1);
        // plane=1: out[0]=B, out[1]=G, out[2]=R
        let b = (0.0 - DET_MEAN[0]) / DET_STD[0];
        let g = (0.0 - DET_MEAN[1]) / DET_STD[1];
        let r = (1.0 - DET_MEAN[2]) / DET_STD[2];
        assert!((out[0] - b).abs() < 1e-6);
        assert!((out[1] - g).abs() < 1e-6);
        assert!((out[2] - r).abs() < 1e-6);
    }

    #[test]
    fn rec_width_clamps_and_pads() {
        // wide short box → clamp to MAX
        let (rw, padded) = rec_width(100_000, 48);
        assert_eq!(rw, REC_MAX_WIDTH);
        assert_eq!(padded, REC_MAX_WIDTH);
        // narrow box → rw floored to 16 (reference max(16, …)), padded to MIN_WIDTH
        let (rw2, padded2) = rec_width(10, 48);
        assert_eq!(rw2, 16);
        assert_eq!(padded2, REC_MIN_WIDTH);
        // proportional case: 100-wide box at height 48 stays 100, pads to 320
        let (rw3, padded3) = rec_width(100, 48);
        assert_eq!(rw3, 100);
        assert_eq!(padded3, REC_MIN_WIDTH);
    }

    #[test]
    fn normalize_rec_zero_pads_right() {
        // rw=1, padded=320, single white pixel column of height 48
        let rw = 1;
        let padded = REC_MIN_WIDTH;
        let rgb = vec![255u8; rw * REC_HEIGHT * 3];
        let out = normalize_rec(&rgb, rw, padded);
        let plane = padded * REC_HEIGHT;
        // filled pixel at (y=0,x=0)
        assert!((out[0] - 1.0).abs() < 1e-6); // 255 → +1
        // padding pixel at (y=0,x=1) stays 0.0 (PP pads with raw 0, not normalized)
        assert_eq!(out[1], 0.0);
        assert_eq!(out[plane + 1], 0.0);
    }
}
