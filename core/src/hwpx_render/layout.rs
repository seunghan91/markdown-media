// Ported from kkdoc (MIT): src/render/layout.ts
//! 레이아웃 보존 렌더 — 순수 계산 유틸 (XML 무접촉). 단위 HWPUNIT(1/7200in).

/// HWPX 좌표 속성은 uint32로 저장된 음수가 섞여 있다 (예: vertOffset=4294967103 = -193)
pub fn to_int32(v: Option<&str>, fallback: f64) -> f64 {
    match v {
        None => fallback,
        Some(s) if s.is_empty() => fallback,
        Some(s) => {
            // 정수 문자열을 f64로 파싱 후 uint32 음수 복원
            match s.trim().parse::<f64>() {
                Ok(n) if n.is_finite() => {
                    if n > 0x7fff_ffff as f64 {
                        n - 0x1_0000_0000u64 as f64
                    } else {
                        n
                    }
                }
                _ => fallback,
            }
        }
    }
}

/// 셀 하나가 주는 경계 제약: boundary[a] + size = boundary[b]
#[derive(Clone, Copy)]
pub struct SpanConstraint {
    pub a: usize,
    pub b: usize,
    pub size: f64,
}

/// 표 열 경계 솔버 — 경계 전파. 미해결 경계는 인접 확정 경계 사이 균등 보간.
pub fn solve_boundaries(constraints: &[SpanConstraint], count: usize, total: Option<f64>) -> Vec<f64> {
    let mut x: Vec<Option<f64>> = vec![None; count + 1];
    x[0] = Some(0.0);
    if let Some(t) = total {
        if t > 0.0 {
            x[count] = Some(t);
        }
    }
    let mut changed = true;
    let mut guard = 0;
    while changed && guard < count + 8 {
        guard += 1;
        changed = false;
        for c in constraints {
            if c.b > count || c.a >= c.b {
                continue;
            }
            match (x[c.a], x[c.b]) {
                (Some(xa), None) => {
                    x[c.b] = Some(xa + c.size);
                    changed = true;
                }
                (None, Some(xb)) => {
                    x[c.a] = Some(xb - c.size);
                    changed = true;
                }
                _ => {}
            }
        }
    }
    // 잔여 미해결 경계 — 좌우 확정 경계 사이 균등 보간
    let mut i = 0;
    while i <= count {
        if x[i].is_some() {
            i += 1;
            continue;
        }
        let lo = i - 1;
        let mut hi = i;
        while hi <= count && x[hi].is_none() {
            hi += 1;
        }
        let lo_v = x[lo].unwrap();
        let hi_v = if hi <= count { x[hi].unwrap() } else { lo_v + ((hi - lo) as f64) * 1000.0 };
        let n = (hi - lo) as f64;
        for kk in 1..(hi - lo) {
            x[lo + kk] = Some(lo_v + (hi_v - lo_v) * (kk as f64) / n);
        }
        if hi > count {
            x[count] = Some(hi_v);
        }
        i = hi;
    }
    // 단조 보정 (모순 제약 방어)
    let mut out: Vec<f64> = x.into_iter().map(|v| v.unwrap_or(0.0)).collect();
    for k in 1..=count {
        if out[k] < out[k - 1] {
            out[k] = out[k - 1];
        }
    }
    out
}

/// 행 높이 솔버 입력 — rowSpan=1 셀의 max가 기본, rowSpan>1은 잔여 균등분배.
pub struct RowCell {
    pub row_addr: usize,
    pub row_span: usize,
    pub height: f64,
    pub content_h: Option<f64>,
}

/// 표 행 높이 솔버.
pub fn solve_row_heights(cells: &[RowCell], row_count: usize) -> Vec<f64> {
    let mut h = vec![0.0_f64; row_count];
    for c in cells {
        if c.row_span == 1 && c.row_addr < row_count {
            h[c.row_addr] = h[c.row_addr].max(c.height);
        }
    }
    // rowSpan>1 셀 — 포함 행 중 미확정(0) 행에 잔여 균등분배
    for c in cells {
        if c.row_span <= 1 {
            continue;
        }
        let end = (c.row_addr + c.row_span).min(row_count);
        let rows: Vec<usize> = (c.row_addr..end).collect();
        let known: f64 = rows.iter().map(|&r| h[r]).sum();
        let missing: Vec<usize> = rows.iter().copied().filter(|&r| h[r] == 0.0).collect();
        if !missing.is_empty() && c.height > known {
            let each = (c.height - known) / (missing.len() as f64);
            for r in missing {
                h[r] = each;
            }
        }
    }
    // 콘텐츠 초과 성장 (rowSpan=1 셀만 — 사진 셀 케이스)
    for c in cells {
        if c.row_span == 1 && c.row_addr < row_count {
            if let Some(ch) = c.content_h {
                if ch > h[c.row_addr] {
                    h[c.row_addr] = ch;
                }
            }
        }
    }
    h
}
