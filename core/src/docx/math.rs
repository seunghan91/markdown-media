//! OMML (Office Math Markup Language) → LaTeX converter.
//!
//! Streaming, stack-based converter invoked from the main DOCX XML walk.
//! Covers the most common math constructs: runs, sub/superscripts, fractions,
//! radicals, delimiters, n-ary operators (sum/int/prod), function apply,
//! accents, bars. Unknown constructs degrade gracefully to their child text.
//!
//! Approach adapted from microsoft/markitdown's `converter_utils/docx/math/`
//! (itself derived from xiilei/dwml) — both MIT-licensed. The logic here is a
//! Rust rewrite, not a copy; LaTeX emission rules are the common reference.

/// A node in the partially-built math expression tree.
#[derive(Debug)]
enum Frame {
    /// `m:oMath` or `m:oMathPara` root — collect inline children.
    Root { children: String },
    /// `m:r` — math run collecting text from `m:t`.
    Run { text: String },
    /// `m:e` — expression slot (used as base/argument in many parents).
    E { children: String },
    /// `m:sub` — subscript slot.
    Sub { children: String },
    /// `m:sup` — superscript slot.
    Sup { children: String },
    /// `m:num` — fraction numerator.
    Num { children: String },
    /// `m:den` — fraction denominator.
    Den { children: String },
    /// `m:deg` — radical degree.
    Deg { children: String },
    /// `m:lim` — limit expression (used under limLow/limUpp).
    Lim { children: String },
    /// `m:fName` — function name for `m:func`.
    FName { children: String },
    /// `m:sSub` — subscript expression: base + sub.
    SSub { e: Option<String>, sub: Option<String> },
    /// `m:sSup` — superscript expression.
    SSup { e: Option<String>, sup: Option<String> },
    /// `m:sSubSup` — base + sub + sup.
    SSubSup { e: Option<String>, sub: Option<String>, sup: Option<String> },
    /// `m:f` — fraction.
    F { num: Option<String>, den: Option<String> },
    /// `m:rad` — radical.
    Rad { e: Option<String>, deg: Option<String> },
    /// `m:d` — delimiter object.
    D { beg: char, end: char, children: String },
    /// `m:nary` — n-ary operator (sum, integral, ...).
    Nary {
        op: char,
        sub: Option<String>,
        sup: Option<String>,
        body: Option<String>,
    },
    /// `m:func` — function apply (sin, cos, ...).
    Func { name: Option<String>, e: Option<String> },
    /// `m:acc` — accent over an expression.
    Acc { ch: char, e: Option<String> },
    /// `m:bar` — bar over/under.
    Bar { pos: BarPos, e: Option<String> },
    /// `m:limLow` — underscript limit: e + lim.
    LimLow { e: Option<String>, lim: Option<String> },
    /// `m:limUpp` — overscript limit.
    LimUpp { e: Option<String>, lim: Option<String> },
    /// `m:groupChr` — character grouping (e.g., overbrace).
    GroupChr { ch: char, e: Option<String> },
    /// Container that just concatenates children (unknown or `*Pr` props).
    PassThrough { children: String },
    /// `*Pr` property container — swallow children, but capture `chr` attr.
    Props { kind: PropsKind, chr: Option<char>, beg: Option<char>, end: Option<char>, pos: Option<String> },
}

#[derive(Debug, Clone, Copy)]
enum BarPos { Top, Bottom }

#[derive(Debug, Clone, Copy)]
enum PropsKind { Nary, D, Acc, Bar, GroupChr, Other }

/// Output kind once finalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathKind {
    /// `$...$` inline (from `m:oMath`).
    Inline,
    /// `$$...$$` display (from `m:oMathPara`).
    Block,
}

/// Streaming OMML parser driven by the DOCX XML walk.
///
/// The caller feeds SAX-like events; the builder maintains an internal stack
/// and, on `finish()`, emits a LaTeX string.
#[derive(Debug)]
pub struct OmmlBuilder {
    kind: MathKind,
    stack: Vec<Frame>,
}

impl OmmlBuilder {
    pub fn new(kind: MathKind) -> Self {
        Self {
            kind,
            stack: vec![Frame::Root { children: String::new() }],
        }
    }

    pub fn kind(&self) -> MathKind { self.kind }

    /// Try to consume a val-carrying child of a Props frame.
    /// Returns true if the tag was consumed (caller must push a sink frame for Start, or skip for Empty).
    fn consume_props_child(&mut self, local_name: &[u8], attrs: &[(Vec<u8>, String)]) -> bool {
        let val = attr_string(attrs, b"val");
        let Some(top) = self.stack.last_mut() else { return false; };
        let Frame::Props { chr, beg, end, pos, .. } = top else { return false; };
        match local_name {
            b"chr"    => { *chr = val.and_then(|s| s.chars().next()); true }
            b"begChr" => { *beg = val.and_then(|s| s.chars().next()); true }
            b"endChr" => { *end = val.and_then(|s| s.chars().next()); true }
            b"pos"    => { *pos = val; true }
            b"type"   => true, // fraction bar/skewed type; ignored in MVP
            _ => false,
        }
    }

    /// Handle a self-closing element (Event::Empty).
    pub fn empty(&mut self, local_name: &[u8], attrs: &[(Vec<u8>, String)]) {
        if self.consume_props_child(local_name, attrs) { return; }
        // Non-Props empty elements: treat as start+end (no text content).
        self.start(local_name, attrs);
        self.end(local_name);
    }

    /// Push a frame for an element start.
    /// `local_name` is the XML local name (e.g. "sSub", "f") with no namespace prefix.
    /// `attrs` is a slice of (local_name, value) pairs.
    pub fn start(&mut self, local_name: &[u8], attrs: &[(Vec<u8>, String)]) {
        // `m:t` holds the text content of the enclosing `m:r` — no dedicated frame;
        // text events will be captured by the nearest enclosing Run frame.
        if local_name == b"t" { return; }
        // Inside a Props frame, child val-carrying elements update the Props directly.
        if self.consume_props_child(local_name, attrs) {
            // Push a sink frame so the matching End event has something to pop.
            self.stack.push(Frame::PassThrough { children: String::new() });
            return;
        }
        let frame = match local_name {
            b"r"         => Frame::Run { text: String::new() },
            b"e"         => Frame::E   { children: String::new() },
            b"sub"       => Frame::Sub { children: String::new() },
            b"sup"       => Frame::Sup { children: String::new() },
            b"num"       => Frame::Num { children: String::new() },
            b"den"       => Frame::Den { children: String::new() },
            b"deg"       => Frame::Deg { children: String::new() },
            b"lim"       => Frame::Lim { children: String::new() },
            b"fName"     => Frame::FName { children: String::new() },
            b"sSub"      => Frame::SSub { e: None, sub: None },
            b"sSup"      => Frame::SSup { e: None, sup: None },
            b"sSubSup"   => Frame::SSubSup { e: None, sub: None, sup: None },
            b"f"         => Frame::F { num: None, den: None },
            b"rad"       => Frame::Rad { e: None, deg: None },
            b"nary"      => Frame::Nary { op: '∑', sub: None, sup: None, body: None },
            b"d"         => Frame::D { beg: '(', end: ')', children: String::new() },
            b"func"      => Frame::Func { name: None, e: None },
            b"acc"       => Frame::Acc { ch: '̂', e: None },
            b"bar"       => Frame::Bar { pos: BarPos::Top, e: None },
            b"limLow"    => Frame::LimLow { e: None, lim: None },
            b"limUpp"    => Frame::LimUpp { e: None, lim: None },
            b"groupChr"  => Frame::GroupChr { ch: '‾', e: None },

            b"naryPr"     => Frame::Props { kind: PropsKind::Nary,     chr: None, beg: None, end: None, pos: None },
            b"dPr"        => Frame::Props { kind: PropsKind::D,        chr: None, beg: None, end: None, pos: None },
            b"accPr"      => Frame::Props { kind: PropsKind::Acc,      chr: None, beg: None, end: None, pos: None },
            b"barPr"      => Frame::Props { kind: PropsKind::Bar,      chr: None, beg: None, end: None, pos: None },
            b"groupChrPr" => Frame::Props { kind: PropsKind::GroupChr, chr: None, beg: None, end: None, pos: None },
            _            => Frame::PassThrough { children: String::new() },
        };
        self.stack.push(frame);
    }

    /// Add text from a `m:t` to the nearest enclosing Run frame (or drop if none).
    pub fn text(&mut self, s: &str) {
        for frame in self.stack.iter_mut().rev() {
            if let Frame::Run { text } = frame {
                text.push_str(s);
                return;
            }
        }
    }

    /// Pop the current frame, render to LaTeX, attach to parent.
    pub fn end(&mut self, local_name: &[u8]) {
        // Ignore trailing `t` close events — they come after run frames already popped.
        if local_name == b"t" { return; }

        let Some(frame) = self.stack.pop() else { return; };

        // If popping a Props frame, push its captured attrs back into parent's slot.
        if let Frame::Props { kind, chr, beg, end, pos } = &frame {
            if let Some(parent) = self.stack.last_mut() {
                match (kind, parent) {
                    (PropsKind::Nary, Frame::Nary { op, .. }) => if let Some(c) = chr { *op = *c; }
                    (PropsKind::D, Frame::D { beg: b, end: e, .. }) => {
                        if let Some(c) = beg { *b = *c; }
                        if let Some(c) = end { *e = *c; }
                    }
                    (PropsKind::Acc, Frame::Acc { ch, .. }) => if let Some(c) = chr { *ch = *c; }
                    (PropsKind::Bar, Frame::Bar { pos: p, .. }) => {
                        if pos.as_deref() == Some("top") { *p = BarPos::Top; }
                        else if pos.as_deref() == Some("bot") { *p = BarPos::Bottom; }
                    }
                    (PropsKind::GroupChr, Frame::GroupChr { ch, .. }) => if let Some(c) = chr { *ch = *c; }
                    _ => {}
                }
            }
            return;
        }

        let rendered = render_frame(&frame);
        self.attach_to_parent(&frame, rendered);
    }

    fn attach_to_parent(&mut self, closed: &Frame, rendered: String) {
        // Named-slot attach: try to place the child into a specific slot on its parent.
        // Returns true if attached, false if caller should fall back to plain concatenation.
        let attached = {
            let Some(parent) = self.stack.last_mut() else { return; };
            match (closed, parent) {
                (Frame::E { .. },   Frame::SSub    { e,   .. }) => { *e   = Some(rendered.clone()); true }
                (Frame::Sub { .. }, Frame::SSub    { sub, .. }) => { *sub = Some(rendered.clone()); true }
                (Frame::E { .. },   Frame::SSup    { e,   .. }) => { *e   = Some(rendered.clone()); true }
                (Frame::Sup { .. }, Frame::SSup    { sup, .. }) => { *sup = Some(rendered.clone()); true }
                (Frame::E { .. },   Frame::SSubSup { e,   .. }) => { *e   = Some(rendered.clone()); true }
                (Frame::Sub { .. }, Frame::SSubSup { sub, .. }) => { *sub = Some(rendered.clone()); true }
                (Frame::Sup { .. }, Frame::SSubSup { sup, .. }) => { *sup = Some(rendered.clone()); true }
                (Frame::Num { .. }, Frame::F { num, .. })       => { *num = Some(rendered.clone()); true }
                (Frame::Den { .. }, Frame::F { den, .. })       => { *den = Some(rendered.clone()); true }
                (Frame::E { .. },   Frame::Rad { e,   .. })     => { *e   = Some(rendered.clone()); true }
                (Frame::Deg { .. }, Frame::Rad { deg, .. })     => { *deg = Some(rendered.clone()); true }
                (Frame::Sub { .. }, Frame::Nary { sub, .. })    => { *sub = Some(rendered.clone()); true }
                (Frame::Sup { .. }, Frame::Nary { sup, .. })    => { *sup = Some(rendered.clone()); true }
                (Frame::E { .. },   Frame::Nary { body, .. })   => { *body = Some(rendered.clone()); true }
                (Frame::FName { .. }, Frame::Func { name, .. }) => { *name = Some(rendered.clone()); true }
                (Frame::E { .. },     Frame::Func { e,    .. }) => { *e    = Some(rendered.clone()); true }
                (Frame::E { .. }, Frame::Acc { e, .. })         => { *e = Some(rendered.clone()); true }
                (Frame::E { .. }, Frame::Bar { e, .. })         => { *e = Some(rendered.clone()); true }
                (Frame::E { .. },   Frame::LimLow { e,   .. })  => { *e   = Some(rendered.clone()); true }
                (Frame::Lim { .. }, Frame::LimLow { lim, .. })  => { *lim = Some(rendered.clone()); true }
                (Frame::E { .. },   Frame::LimUpp { e,   .. })  => { *e   = Some(rendered.clone()); true }
                (Frame::Lim { .. }, Frame::LimUpp { lim, .. })  => { *lim = Some(rendered.clone()); true }
                (Frame::E { .. }, Frame::GroupChr { e, .. })    => { *e = Some(rendered.clone()); true }
                _ => false,
            }
        };
        if attached { return; }
        // Fall back: append to a plain children buffer on the parent.
        if let Some(parent) = self.stack.last_mut() {
            match parent {
                Frame::Root { children }
                | Frame::E { children }
                | Frame::Sub { children }
                | Frame::Sup { children }
                | Frame::Num { children }
                | Frame::Den { children }
                | Frame::Deg { children }
                | Frame::Lim { children }
                | Frame::FName { children }
                | Frame::PassThrough { children }
                | Frame::D { children, .. } => children.push_str(&rendered),
                _ => {}
            }
        }
    }

    /// Consume the builder and produce the final LaTeX with delimiters.
    pub fn finish(mut self) -> String {
        // Pop everything down to Root.
        while self.stack.len() > 1 {
            let top = self.stack.pop().unwrap();
            let rendered = render_frame(&top);
            self.attach_to_parent(&top, rendered);
        }
        let inner = match self.stack.pop() {
            Some(Frame::Root { children }) => children,
            _ => String::new(),
        };
        let trimmed = inner.trim();
        match self.kind {
            MathKind::Inline => format!("${}$", trimmed),
            MathKind::Block  => format!("$${}$$", trimmed),
        }
    }
}

fn attr_string(attrs: &[(Vec<u8>, String)], name: &[u8]) -> Option<String> {
    for (k, v) in attrs {
        if k.as_slice() == name {
            return Some(v.clone());
        }
    }
    None
}

fn render_frame(frame: &Frame) -> String {
    match frame {
        Frame::Root { children } => children.clone(),
        Frame::Run { text } => escape_latex(text),
        Frame::E { children }
        | Frame::Sub { children }
        | Frame::Sup { children }
        | Frame::Num { children }
        | Frame::Den { children }
        | Frame::Deg { children }
        | Frame::Lim { children }
        | Frame::FName { children }
        | Frame::PassThrough { children } => children.clone(),

        Frame::SSub { e, sub } => format!(
            "{{{}}}_{{{}}}",
            e.as_deref().unwrap_or(""),
            sub.as_deref().unwrap_or(""),
        ),
        Frame::SSup { e, sup } => format!(
            "{{{}}}^{{{}}}",
            e.as_deref().unwrap_or(""),
            sup.as_deref().unwrap_or(""),
        ),
        Frame::SSubSup { e, sub, sup } => format!(
            "{{{}}}_{{{}}}^{{{}}}",
            e.as_deref().unwrap_or(""),
            sub.as_deref().unwrap_or(""),
            sup.as_deref().unwrap_or(""),
        ),
        Frame::F { num, den } => format!(
            "\\frac{{{}}}{{{}}}",
            num.as_deref().unwrap_or(""),
            den.as_deref().unwrap_or(""),
        ),
        Frame::Rad { e, deg } => match deg.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(d) => format!("\\sqrt[{}]{{{}}}", d, e.as_deref().unwrap_or("")),
            None => format!("\\sqrt{{{}}}", e.as_deref().unwrap_or("")),
        },
        Frame::D { beg, end, children } => format!(
            "\\left{} {} \\right{}",
            latex_delim(*beg),
            children,
            latex_delim(*end),
        ),
        Frame::Nary { op, sub, sup, body } => {
            let op_tex = nary_op(*op);
            let mut out = op_tex.to_string();
            if let Some(s) = sub.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                out.push_str(&format!("_{{{}}}", s));
            }
            if let Some(s) = sup.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                out.push_str(&format!("^{{{}}}", s));
            }
            if let Some(b) = body.as_deref() {
                out.push(' ');
                out.push_str(b);
            }
            out
        }
        Frame::Func { name, e } => format!(
            "{}{{{}}}",
            name.as_deref().unwrap_or(""),
            e.as_deref().unwrap_or(""),
        ),
        Frame::Acc { ch, e } => format!(
            "{}{{{}}}",
            accent_tex(*ch),
            e.as_deref().unwrap_or(""),
        ),
        Frame::Bar { pos, e } => match pos {
            BarPos::Top    => format!("\\overline{{{}}}", e.as_deref().unwrap_or("")),
            BarPos::Bottom => format!("\\underline{{{}}}", e.as_deref().unwrap_or("")),
        },
        Frame::LimLow { e, lim } => format!(
            "{}_{{{}}}",
            e.as_deref().unwrap_or(""),
            lim.as_deref().unwrap_or(""),
        ),
        Frame::LimUpp { e, lim } => format!(
            "{}^{{{}}}",
            e.as_deref().unwrap_or(""),
            lim.as_deref().unwrap_or(""),
        ),
        Frame::GroupChr { ch, e } => format!(
            "{}{{{}}}",
            group_chr_tex(*ch),
            e.as_deref().unwrap_or(""),
        ),
        Frame::Props { .. } => String::new(),
    }
}

fn escape_latex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\backslash "),
            '{'  => out.push_str("\\{"),
            '}'  => out.push_str("\\}"),
            '$'  => out.push_str("\\$"),
            '%'  => out.push_str("\\%"),
            '&'  => out.push_str("\\&"),
            '#'  => out.push_str("\\#"),
            '_'  => out.push_str("\\_"),
            '^'  => out.push_str("\\^{}"),
            '~'  => out.push_str("\\sim "),
            _    => out.push(c),
        }
    }
    out
}

fn latex_delim(c: char) -> &'static str {
    match c {
        '(' => "(",
        ')' => ")",
        '[' => "[",
        ']' => "]",
        '{' => "\\{",
        '}' => "\\}",
        '|' => "|",
        '⟨' => "\\langle",
        '⟩' => "\\rangle",
        '‖' => "\\|",
        _   => ".",
    }
}

fn nary_op(c: char) -> &'static str {
    match c {
        '∑' => "\\sum",
        '∏' => "\\prod",
        '∐' => "\\coprod",
        '∫' => "\\int",
        '∬' => "\\iint",
        '∭' => "\\iiint",
        '∮' => "\\oint",
        '⋀' => "\\bigwedge",
        '⋁' => "\\bigvee",
        '⋂' => "\\bigcap",
        '⋃' => "\\bigcup",
        _   => "\\sum",
    }
}

fn accent_tex(c: char) -> &'static str {
    match c {
        '̂' | '^' => "\\hat",
        '̃' | '~' => "\\tilde",
        '̄' | '‾' => "\\bar",
        '̇' => "\\dot",
        '̈' => "\\ddot",
        '́' => "\\acute",
        '̀' => "\\grave",
        '̌' => "\\check",
        '̆' => "\\breve",
        '⃗' => "\\vec",
        _   => "\\hat",
    }
}

fn group_chr_tex(c: char) -> &'static str {
    match c {
        '⏞' => "\\overbrace",
        '⏟' => "\\underbrace",
        '‾' => "\\overline",
        '_' => "\\underline",
        _   => "\\overline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(events: &[(&str, &str, &[(&str, &str)])]) -> String {
        // events: ("start"|"end"|"text"|"empty", name, attrs)
        let mut b = OmmlBuilder::new(MathKind::Inline);
        for (kind, name, attrs) in events {
            let attr_vec: Vec<(Vec<u8>, String)> = attrs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.to_string()))
                .collect();
            match *kind {
                "start" => b.start(name.as_bytes(), &attr_vec),
                "end"   => b.end(name.as_bytes()),
                "empty" => b.empty(name.as_bytes(), &attr_vec),
                "text"  => b.text(name),
                _ => panic!("bad kind"),
            }
        }
        b.finish()
    }

    #[test]
    fn plain_run() {
        // $x$
        let out = run(&[
            ("start", "r", &[]),
            ("text",  "x", &[]),
            ("end",   "r", &[]),
        ]);
        assert_eq!(out, "$x$");
    }

    #[test]
    fn fraction() {
        // \frac{a}{b}
        let out = run(&[
            ("start", "f",   &[]),
            ("start", "num", &[]),
            ("start", "r",   &[]),
            ("text",  "a",   &[]),
            ("end",   "r",   &[]),
            ("end",   "num", &[]),
            ("start", "den", &[]),
            ("start", "r",   &[]),
            ("text",  "b",   &[]),
            ("end",   "r",   &[]),
            ("end",   "den", &[]),
            ("end",   "f",   &[]),
        ]);
        assert_eq!(out, "$\\frac{a}{b}$");
    }

    #[test]
    fn superscript() {
        // x^{2}
        let out = run(&[
            ("start", "sSup", &[]),
            ("start", "e",    &[]),
            ("start", "r",    &[]),
            ("text",  "x",    &[]),
            ("end",   "r",    &[]),
            ("end",   "e",    &[]),
            ("start", "sup",  &[]),
            ("start", "r",    &[]),
            ("text",  "2",    &[]),
            ("end",   "r",    &[]),
            ("end",   "sup",  &[]),
            ("end",   "sSup", &[]),
        ]);
        assert_eq!(out, "${x}^{2}$");
    }

    #[test]
    fn subscript() {
        let out = run(&[
            ("start","sSub",&[]),
            ("start","e",&[]), ("start","r",&[]), ("text","a",&[]), ("end","r",&[]), ("end","e",&[]),
            ("start","sub",&[]), ("start","r",&[]), ("text","1",&[]), ("end","r",&[]), ("end","sub",&[]),
            ("end","sSub",&[]),
        ]);
        assert_eq!(out, "${a}_{1}$");
    }

    #[test]
    fn subsup() {
        let out = run(&[
            ("start","sSubSup",&[]),
            ("start","e",&[]), ("start","r",&[]), ("text","x",&[]), ("end","r",&[]), ("end","e",&[]),
            ("start","sub",&[]), ("start","r",&[]), ("text","0",&[]), ("end","r",&[]), ("end","sub",&[]),
            ("start","sup",&[]), ("start","r",&[]), ("text","n",&[]), ("end","r",&[]), ("end","sup",&[]),
            ("end","sSubSup",&[]),
        ]);
        assert_eq!(out, "${x}_{0}^{n}$");
    }

    #[test]
    fn radical_default() {
        let out = run(&[
            ("start","rad",&[]),
            ("start","e",&[]), ("start","r",&[]), ("text","x",&[]), ("end","r",&[]), ("end","e",&[]),
            ("end","rad",&[]),
        ]);
        assert_eq!(out, "$\\sqrt{x}$");
    }

    #[test]
    fn radical_with_degree() {
        let out = run(&[
            ("start","rad",&[]),
            ("start","deg",&[]), ("start","r",&[]), ("text","3",&[]), ("end","r",&[]), ("end","deg",&[]),
            ("start","e",&[]), ("start","r",&[]), ("text","y",&[]), ("end","r",&[]), ("end","e",&[]),
            ("end","rad",&[]),
        ]);
        assert_eq!(out, "$\\sqrt[3]{y}$");
    }

    #[test]
    fn nary_sum_with_limits() {
        // \sum_{i=0}^{n} i — chr is an empty child of naryPr with val attr
        let out = run(&[
            ("start","nary",&[]),
            ("start","naryPr",&[]),
                ("empty","chr",&[("val","∑")]),
            ("end","naryPr",&[]),
            ("start","sub",&[]), ("start","r",&[]), ("text","i=0",&[]), ("end","r",&[]), ("end","sub",&[]),
            ("start","sup",&[]), ("start","r",&[]), ("text","n",&[]), ("end","r",&[]), ("end","sup",&[]),
            ("start","e",&[]), ("start","r",&[]), ("text","i",&[]), ("end","r",&[]), ("end","e",&[]),
            ("end","nary",&[]),
        ]);
        assert_eq!(out, "$\\sum_{i=0}^{n} i$");
    }

    #[test]
    fn delimiter_parens() {
        // (x) — begChr/endChr are empty children of dPr
        let out = run(&[
            ("start","d",&[]),
            ("start","dPr",&[]),
                ("empty","begChr",&[("val","(")]),
                ("empty","endChr",&[("val",")")]),
            ("end","dPr",&[]),
            ("start","e",&[]), ("start","r",&[]), ("text","x",&[]), ("end","r",&[]), ("end","e",&[]),
            ("end","d",&[]),
        ]);
        assert_eq!(out, "$\\left( x \\right)$");
    }

    #[test]
    fn block_math() {
        let mut b = OmmlBuilder::new(MathKind::Block);
        b.start(b"r", &[]);
        b.text("E=mc^2");
        b.end(b"r");
        let out = b.finish();
        assert!(out.starts_with("$$") && out.ends_with("$$"));
    }

    #[test]
    fn latex_escape_safe_chars() {
        let out = run(&[
            ("start","r",&[]),
            ("text","a_b & c",&[]),
            ("end","r",&[]),
        ]);
        assert_eq!(out, "$a\\_b \\& c$");
    }

    #[test]
    fn unknown_tag_passes_text_through() {
        // Unknown wrapper degrades to its child text.
        let out = run(&[
            ("start","weirdWrapper",&[]),
            ("start","r",&[]), ("text","q",&[]), ("end","r",&[]),
            ("end","weirdWrapper",&[]),
        ]);
        assert_eq!(out, "$q$");
    }
}
