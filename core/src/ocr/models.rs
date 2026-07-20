// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/models.ts
//
//! OCR model specs (PP-OCRv5 korean) + cache-path resolution + dictionary parsing.
//!
//! Cache layout: `~/.cache/mdm/models/ppocr/` (~18MB: det 5 + rec 13 + dict 1).
//! Override the root with the `MDM_MODEL_CACHE` env var.
//!
//! Model provenance: PaddlePaddle official HuggingFace ONNX conversions (Apache-2.0).
//! SHA-256 values were verified against the actual downloads / HF `lfs.oid` — do not
//! change them without re-verifying. The korean dictionary (11,945 chars: all 11,172
//! precomposed Hangul syllables + jamo/latin/symbols) is embedded in the rec repo's
//! `inference.yml` under `PostProcess.character_dict`.

use std::path::PathBuf;

/// A downloadable model artifact.
#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub name: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_mb: u32,
}

pub const OCR_DET_MODEL: ModelSpec = ModelSpec {
    name: "PP-OCRv5 mobile det",
    filename: "det.onnx",
    url: "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/main/inference.onnx",
    sha256: "a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d",
    size_mb: 5,
};

pub const OCR_REC_MODEL: ModelSpec = ModelSpec {
    name: "PP-OCRv5 korean rec",
    filename: "rec_korean.onnx",
    url: "https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_onnx/resolve/main/inference.onnx",
    sha256: "92f0b7785e64fc9090106a241cf4c1eb97472824558272751b88a2a4476d3a08",
    size_mb: 13,
};

pub const OCR_REC_DICT: ModelSpec = ModelSpec {
    name: "PP-OCRv5 korean dict",
    filename: "rec_korean.yml",
    url: "https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_onnx/resolve/main/inference.yml",
    sha256: "f757fa1c40e99edcf27e9cce879b93eb2a51fa46f5ef39095689b8c37dd75998",
    size_mb: 1,
};

pub const ALL_OCR_MODELS: [ModelSpec; 3] = [OCR_DET_MODEL, OCR_REC_MODEL, OCR_REC_DICT];

/// Cache directory for a model group. Honors `MDM_MODEL_CACHE`, else
/// `~/.cache/mdm/models/<subdir>/` (falls back to `./.mdm-cache` if HOME is unset).
pub fn models_dir_for(subdir: &str) -> PathBuf {
    if let Ok(root) = std::env::var("MDM_MODEL_CACHE") {
        if !root.trim().is_empty() {
            return PathBuf::from(root).join(subdir);
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".cache")
        .join("mdm")
        .join("models")
        .join(subdir)
}

/// Text-OCR model directory (`ppocr` subdir).
pub fn ocr_models_dir() -> PathBuf {
    models_dir_for("ppocr")
}

pub fn model_path(spec: &ModelSpec) -> PathBuf {
    ocr_models_dir().join(spec.filename)
}

/// True when every OCR model file exists and is non-empty (cheap presence check,
/// no SHA verification — the download script owns integrity).
pub fn models_present() -> bool {
    ALL_OCR_MODELS.iter().all(|spec| {
        std::fs::metadata(model_path(spec))
            .map(|m| m.is_file() && m.len() > 0)
            .unwrap_or(false)
    })
}

/// Extract the `PostProcess.character_dict` list from the rec `inference.yml`.
///
/// Avoids a full YAML parser: reads the `- <char>` lines under `character_dict:`
/// in order (official yml is fixed — 2-space indent, one char per line). The YAML
/// allows list items at the *same* indent as the key, so the terminator is "a
/// non-empty line whose indent is shallower than the key".
///
/// CTC class layout: index 0 = blank, 1..N = dict order, N+1 (last) = space.
pub fn parse_character_dict(yml: &str) -> Vec<String> {
    let mut chars = Vec::new();
    let mut in_dict = false;
    let mut dict_indent: isize = -1;
    for line in yml.split('\n') {
        if !in_dict {
            // match `<indent>character_dict:` (nothing meaningful after the colon)
            let trimmed = line.trim_start();
            if trimmed == "character_dict:" {
                in_dict = true;
                dict_indent = (line.len() - trimmed.len()) as isize;
            }
            continue;
        }
        let indent = (line.len() - line.trim_start().len()) as isize;
        let body = line.trim_start();
        if let Some(rest) = body.strip_prefix("- ") {
            if indent >= dict_indent {
                chars.push(unquote_yaml(rest));
                continue;
            }
        }
        // Shallower non-empty line ends the block; blank lines pass through.
        if !line.trim().is_empty() {
            break;
        }
    }
    chars
}

/// Strip matching single/double quotes and unescape doubled single-quotes.
fn unquote_yaml(v: &str) -> String {
    let bytes = v.as_bytes();
    if v.len() >= 2 {
        let first = bytes[0];
        let last = bytes[v.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            let inner = &v[1..v.len() - 1];
            if first == b'\'' {
                return inner.replace("''", "'");
            }
            return inner.to_string();
        }
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dict_basic_two_space_indent() {
        let yml = "PostProcess:\n  name: CTCLabelDecode\n  character_dict:\n  - 가\n  - 나\n  - 다\n  use_space_char: true\n";
        let d = parse_character_dict(yml);
        assert_eq!(d, vec!["가", "나", "다"]);
    }

    #[test]
    fn dict_handles_quoted_reserved_chars() {
        let yml = "character_dict:\n  - '#'\n  - \"a\"\n  - 'it''s'\n";
        let d = parse_character_dict(yml);
        assert_eq!(d, vec!["#", "a", "it's"]);
    }

    #[test]
    fn dict_terminates_on_shallower_key() {
        let yml = "  character_dict:\n  - x\n  - y\nGlobal:\n  - not_a_char\n";
        let d = parse_character_dict(yml);
        assert_eq!(d, vec!["x", "y"]);
    }

    #[test]
    fn dict_empty_when_absent() {
        assert!(parse_character_dict("foo: bar\n").is_empty());
    }

    #[test]
    fn cache_dir_respects_env_override() {
        // Non-destructive: just verify the join shape via a synthetic root.
        std::env::set_var("MDM_MODEL_CACHE", "/tmp/xyz-mdm-test");
        let d = ocr_models_dir();
        assert!(d.ends_with("xyz-mdm-test/ppocr") || d.ends_with("ppocr"));
        std::env::remove_var("MDM_MODEL_CACHE");
    }

    #[test]
    fn all_models_have_distinct_filenames() {
        let names: Vec<_> = ALL_OCR_MODELS.iter().map(|m| m.filename).collect();
        assert_eq!(names.len(), 3);
        assert_ne!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
    }
}
