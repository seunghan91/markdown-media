// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/engine.ts (componentBoxes + detect unclip/scale)
//
//! DB post-processing: binarize the det probability map, extract 4-connected
//! components as axis-aligned boxes, then unclip (re-expand) and scale back to
//! the original image. All pure — unit-tested without models.

pub const DET_THRESH: f32 = 0.3;
pub const DET_BOX_THRESH: f32 = 0.6;
pub const DET_UNCLIP_RATIO: f64 = 1.5;
pub const DET_MIN_SIZE: usize = 3;
pub const DET_MAX_BOXES: usize = 1000;

/// A connected component in the (dw × dh) probability-map grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawBox {
    pub x1: usize,
    pub y1: usize,
    pub x2: usize,
    pub y2: usize,
    /// mean probability over the component.
    pub score: f32,
}

/// A detected box in original-image pixels (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// 4-connected components of `prob > DET_THRESH`, each as a bbox with mean score.
/// Keeps components whose score >= DET_BOX_THRESH, sorted top→bottom, left→right.
pub fn component_boxes(prob: &[f32], w: usize, h: usize) -> Vec<RawBox> {
    let n = w * h;
    debug_assert!(prob.len() >= n);
    let mut visited = vec![0u8; n];
    let mut boxes: Vec<RawBox> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for start in 0..n {
        if visited[start] == 1 || prob[start] <= DET_THRESH {
            continue;
        }
        let (mut x1, mut x2) = (start % w, start % w);
        let (mut y1, mut y2) = (start / w, start / w);
        let mut sum = 0.0f64;
        let mut count = 0usize;
        stack.clear();
        stack.push(start);
        visited[start] = 1;
        while let Some(p) = stack.pop() {
            let px = p % w;
            let py = p / w;
            sum += prob[p] as f64;
            count += 1;
            x1 = x1.min(px);
            x2 = x2.max(px);
            y1 = y1.min(py);
            y2 = y2.max(py);
            // 4-neighbours
            if px > 0 && visited[p - 1] == 0 && prob[p - 1] > DET_THRESH {
                visited[p - 1] = 1;
                stack.push(p - 1);
            }
            if px < w - 1 && visited[p + 1] == 0 && prob[p + 1] > DET_THRESH {
                visited[p + 1] = 1;
                stack.push(p + 1);
            }
            if py > 0 && visited[p - w] == 0 && prob[p - w] > DET_THRESH {
                visited[p - w] = 1;
                stack.push(p - w);
            }
            if py < h - 1 && visited[p + w] == 0 && prob[p + w] > DET_THRESH {
                visited[p + w] = 1;
                stack.push(p + w);
            }
        }
        // Drop only when BOTH dims are below the minimum (matches reference).
        if (x2 - x1 + 1) < DET_MIN_SIZE && (y2 - y1 + 1) < DET_MIN_SIZE {
            continue;
        }
        boxes.push(RawBox {
            x1,
            y1,
            x2,
            y2,
            score: (sum / count as f64) as f32,
        });
    }

    boxes.retain(|b| b.score >= DET_BOX_THRESH);
    boxes.sort_by(|a, b| a.y1.cmp(&b.y1).then(a.x1.cmp(&b.x1)));
    boxes
}

/// Unclip each raw box (DB shrinks text regions during training, so re-expand by
/// `delta = area*ratio / perimeter`) and scale from det grid (dw×dh) back to the
/// original (img_w×img_h). Returns at most `DET_MAX_BOXES` valid boxes.
pub fn unclip_and_scale(
    raw: &[RawBox],
    img_w: u32,
    img_h: u32,
    dw: usize,
    dh: usize,
) -> Vec<DetBox> {
    let sx = img_w as f64 / dw.max(1) as f64;
    let sy = img_h as f64 / dh.max(1) as f64;
    let mut out = Vec::new();
    for rb in raw.iter().take(DET_MAX_BOXES) {
        let bw = (rb.x2 - rb.x1 + 1) as f64;
        let bh = (rb.y2 - rb.y1 + 1) as f64;
        let delta = (bw * bh * DET_UNCLIP_RATIO) / (2.0 * (bw + bh));
        let x1 = (((rb.x1 as f64 - delta) * sx).floor()).max(0.0) as u32;
        let y1 = (((rb.y1 as f64 - delta) * sy).floor()).max(0.0) as u32;
        let x2 = (((rb.x2 as f64 + 1.0 + delta) * sx).ceil()).min(img_w as f64) as u32;
        let y2 = (((rb.y2 as f64 + 1.0 + delta) * sy).ceil()).min(img_h as f64) as u32;
        if x2.saturating_sub(x1) < DET_MIN_SIZE as u32 || y2.saturating_sub(y1) < DET_MIN_SIZE as u32
        {
            continue;
        }
        out.push(DetBox {
            x: x1,
            y: y1,
            w: x2 - x1,
            h: y2 - y1,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a w×h map with a filled rectangle at prob=1.0.
    fn map_with_rect(w: usize, h: usize, rx: usize, ry: usize, rw: usize, rh: usize) -> Vec<f32> {
        let mut m = vec![0.0f32; w * h];
        for y in ry..ry + rh {
            for x in rx..rx + rw {
                m[y * w + x] = 1.0;
            }
        }
        m
    }

    #[test]
    fn single_component_bbox() {
        let m = map_with_rect(20, 20, 4, 5, 8, 6);
        let boxes = component_boxes(&m, 20, 20);
        assert_eq!(boxes.len(), 1);
        let b = boxes[0];
        assert_eq!((b.x1, b.y1, b.x2, b.y2), (4, 5, 11, 10));
        assert!((b.score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn two_separated_components() {
        let mut m = map_with_rect(30, 10, 1, 1, 5, 5);
        for y in 1..6 {
            for x in 20..26 {
                m[y * 30 + x] = 1.0;
            }
        }
        let boxes = component_boxes(&m, 30, 10);
        assert_eq!(boxes.len(), 2);
        // sorted left→right
        assert!(boxes[0].x1 < boxes[1].x1);
    }

    #[test]
    fn low_score_component_filtered() {
        // A blob all at prob 0.4: above THRESH (0.3) so it forms a component,
        // but mean score 0.4 < BOX_THRESH (0.6) → dropped.
        let mut m = vec![0.0f32; 100];
        for y in 2..8 {
            for x in 2..8 {
                m[y * 10 + x] = 0.4;
            }
        }
        assert!(component_boxes(&m, 10, 10).is_empty());
    }

    #[test]
    fn below_thresh_is_background() {
        let m = vec![0.2f32; 100]; // all <= THRESH
        assert!(component_boxes(&m, 10, 10).is_empty());
    }

    #[test]
    fn unclip_scales_and_expands() {
        let raw = [RawBox { x1: 10, y1: 10, x2: 30, y2: 14, score: 0.9 }];
        // det grid 100x100 → original 200x100: sx=2, sy=1
        let boxes = unclip_and_scale(&raw, 200, 100, 100, 100);
        assert_eq!(boxes.len(), 1);
        let b = boxes[0];
        // x scaled by 2 and expanded outward on both sides
        assert!(b.x < 20);
        assert!(b.x + b.w > 60);
        assert!(b.y < 10);
    }

    #[test]
    fn unclip_respects_image_bounds() {
        let raw = [RawBox { x1: 0, y1: 0, x2: 99, y2: 99, score: 0.9 }];
        let boxes = unclip_and_scale(&raw, 100, 100, 100, 100);
        let b = boxes[0];
        assert!(b.x + b.w <= 100);
        assert!(b.y + b.h <= 100);
    }
}
