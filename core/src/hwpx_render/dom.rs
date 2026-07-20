// Ported from kkdoc (MIT): XML helpers from src/render/svg-render.ts & parser-shared.ts
//! roxmltree 기반 DOM 헬퍼 — TS의 ln/elements/num/findChildByLocalName/findFirst 상응.
//! HWPX는 네임스페이스 접두사(hp:/hh:/hc:)를 root에 선언하므로 local-name 매칭을 쓴다.

use super::layout::to_int32;
use roxmltree::Node;

/// 요소의 local name (접두사 제거) — TS `ln()`
pub fn ln<'a>(n: &Node<'a, 'a>) -> &'a str {
    n.tag_name().name()
}

/// 자식 요소 이터레이터 — TS `elements()`
pub fn elements<'a, 'input>(n: Node<'a, 'input>) -> impl Iterator<Item = Node<'a, 'input>> {
    n.children().filter(|c| c.is_element())
}

/// 속성값 (local-name; HWPX 좌표 속성은 모두 무접두사)
pub fn attr<'a>(n: Node<'a, 'a>, name: &str) -> Option<&'a str> {
    n.attribute(name)
}

/// 정수 속성값 (uint32 음수 복원) — TS `num()`
pub fn num(n: Option<Node>, name: &str, fallback: f64) -> f64 {
    to_int32(n.and_then(|nd| nd.attribute(name)), fallback)
}

/// 직계 자식 중 local name 일치 첫 요소 — TS `findChildByLocalName()`
pub fn find_child_local<'a, 'input>(n: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

/// 서브트리에서 local name 일치 첫 요소를 재귀 탐색 — TS `findFirst()` / `findDeep()`
pub fn find_first<'a, 'input>(n: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    find_first_depth(n, name, 0)
}

fn find_first_depth<'a, 'input>(n: Node<'a, 'input>, name: &str, depth: u32) -> Option<Node<'a, 'input>> {
    if depth > 64 {
        return None;
    }
    for ch in elements(n) {
        if ch.tag_name().name() == name {
            return Some(ch);
        }
        if let Some(f) = find_first_depth(ch, name, depth + 1) {
            return Some(f);
        }
    }
    None
}

/// XML 이스케이프 — TS `escapeXml()`
pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}
