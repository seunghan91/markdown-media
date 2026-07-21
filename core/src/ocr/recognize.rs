// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/engine.ts (ctcDecode)
//
//! CTC greedy decode of the rec model logits. Pure — unit-tested without models.

/// Greedy CTC decode: per-timestep argmax → collapse consecutive repeats →
/// drop blank (class 0) → map dict (1..=N → dict[c-1], N+1 → space).
///
/// `data` is the flat `[T, C]` logits (row-major). Confidence is the mean of the
/// kept steps' probabilities; if the model emits raw logits (value > 1) the step
/// is softmax-normalized on the fly. Returns `None` when nothing decodes.
pub fn ctc_decode(data: &[f32], t: usize, c: usize, dict: &[String]) -> Option<(String, f32)> {
    if t == 0 || c == 0 || data.len() < t * c {
        return None;
    }
    let mut text = String::new();
    let mut conf_sum = 0.0f64;
    let mut conf_count = 0usize;
    let mut prev: isize = -1;

    for step in 0..t {
        let off = step * c;
        let row = &data[off..off + c];
        // argmax
        let mut best = 0usize;
        let mut best_v = row[0];
        for (j, &v) in row.iter().enumerate().skip(1) {
            if v > best_v {
                best_v = v;
                best = j;
            }
        }
        let repeat = best as isize == prev;
        prev = best as isize;
        if best == 0 || repeat {
            continue;
        }
        // Confidence: softmax prob of the max class if logits aren't probabilities.
        let p = if !(0.0..=1.0001).contains(&best_v) {
            let mut denom = 0.0f64;
            for &v in row {
                denom += ((v - best_v) as f64).exp();
            }
            if denom > 0.0 {
                1.0 / denom
            } else {
                0.0
            }
        } else {
            best_v as f64
        };
        conf_sum += p;
        conf_count += 1;

        if best >= 1 && best <= dict.len() {
            text.push_str(&dict[best - 1]);
        } else if best == dict.len() + 1 {
            text.push(' ');
        }
    }

    if text.is_empty() {
        return None;
    }
    let confidence = if conf_count > 0 {
        (conf_sum / conf_count as f64) as f32
    } else {
        0.0
    };
    Some((text, confidence))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict3() -> Vec<String> {
        vec!["가".to_string(), "나".to_string(), "다".to_string()]
    }

    /// One-hot logit row selecting class `cls` (prob=1.0 so no softmax path).
    fn onehot(rows: &[usize], c: usize) -> Vec<f32> {
        let mut d = vec![0.0f32; rows.len() * c];
        for (i, &cls) in rows.iter().enumerate() {
            d[i * c + cls] = 1.0;
        }
        d
    }

    #[test]
    fn collapses_repeats_and_blanks() {
        // classes: 1,1,0,2,2 → dict[0], dict[1] = "가나"
        let c = 5; // 0 blank, 1..3 dict, 4 space
        let d = onehot(&[1, 1, 0, 2, 2], c);
        let (text, conf) = ctc_decode(&d, 5, c, &dict3()).unwrap();
        assert_eq!(text, "가나");
        assert!((conf - 1.0).abs() < 1e-6);
    }

    #[test]
    fn maps_space_class() {
        let c = 5; // space = dict.len()+1 = 4
        let d = onehot(&[1, 4, 3], c);
        let (text, _) = ctc_decode(&d, 3, c, &dict3()).unwrap();
        assert_eq!(text, "가 다");
    }

    #[test]
    fn all_blank_returns_none() {
        let c = 5;
        let d = onehot(&[0, 0, 0], c);
        assert!(ctc_decode(&d, 3, c, &dict3()).is_none());
    }

    #[test]
    fn softmax_confidence_for_raw_logits() {
        // Row with a dominant logit >1 triggers softmax normalization.
        let c = 5;
        let mut d = vec![0.0f32; c];
        d[1] = 10.0; // huge logit for class 1 → prob ≈ 1
        let (text, conf) = ctc_decode(&d, 1, c, &dict3()).unwrap();
        assert_eq!(text, "가");
        assert!(conf > 0.9 && conf <= 1.0);
    }

    #[test]
    fn guards_bad_shapes() {
        assert!(ctc_decode(&[], 0, 0, &dict3()).is_none());
        assert!(ctc_decode(&[1.0, 2.0], 5, 5, &dict3()).is_none()); // data too short
    }
}
