// Ported from kkdoc (MIT): src/hwpx/equation.ts (CONVERT_MAP / MIDDLE_CONVERT_MAP /
// BAR_CONVERT_MAP / MATRIX_CONVERT_MAP / BRACE_CONVERT_MAP)
//
// Single-token / structural-token replacement tables shared by both
// directions of the HULK <-> LaTeX conversion (to_latex.rs reads them,
// to_hulk.rs builds its reverse index from them).

use std::collections::HashMap;

use lazy_static::lazy_static;

/// `matrix`/`pmatrix`/`bmatrix`/`dmatrix`/`cases`/`eqalign` expansion target.
pub struct MatrixMapping {
    pub begin: &'static str,
    pub end: &'static str,
    pub remove_outer_brackets: bool,
}

// Single-token replacements (applied to each whitespace-delimited token).
pub static CONVERT_MAP_ENTRIES: &[(&str, &str)] = &[
    ("TIMES", "\\times"), ("times", "\\times"),
    ("LEFT", "\\left"), ("RIGHT", "\\right"),
    ("under", "\\underline"),
    ("SMALLSUM", "\\sum"), ("sum", "\\sum"),
    ("SMALLPROD", "\\prod"), ("prod", "\\prod"),
    ("SMALLINTER", "\\cap"),
    ("CUP", "\\cup"),
    ("OPLUS", "\\oplus"), ("OMINUS", "\\ominus"), ("OTIMES", "\\otimes"), ("ODIV", "\\oslash"), ("ODOT", "\\odot"),
    ("LOR", "\\lor"), ("LAND", "\\land"),
    ("SUBSET", "\\subset"), ("SUPERSET", "\\supset"), ("SUBSETEQ", "\\subseteq"), ("SUPSETEQ", "\\supseteq"),
    ("IN", "\\in"), ("OWNS", "\\owns"), ("NOTIN", "\\notin"),
    ("LEQ", "\\leq"), ("GEQ", "\\geq"),
    ("<<", "\\ll"), (">>", "\\gg"), ("<<<", "\\lll"), (">>>", "\\ggg"),
    ("PREC", "\\prec"), ("SUCC", "\\succ"),
    ("UPLUS", "\\uplus"),
    ("\u{00b1}", "\\pm"), ("+-", "\\pm"), ("-+", "\\mp"), ("\u{00f7}", "\\div"),
    ("cdot", "\\cdot"),
    ("CIRC", "\\circ"), ("BULLET", "\\bullet"), ("DEG", " ^\\circ"),
    ("AST", "\\ast"), ("STAR", "\\bigstar"), ("BIGCIRC", "\\bigcirc"),
    ("EMPTYSET", "\\emptyset"),
    ("THEREFORE", "\\therefore"), ("BECAUSE", "\\because"), ("EXIST", "\\exists"),
    ("!=", "\\neq"),
    ("SMCOPROD", "\\coprod"), ("coprod", "\\coprod"),
    ("SQCAP", "\\sqcap"), ("SQCUP", "\\sqcup"),
    ("SQSUBSET", "\\sqsubset"), ("SQSUBSETEQ", "\\sqsubseteq"),
    ("BIGSQCUP", "\\bigsqcup"),
    ("BIGOPLUS", "\\bigoplus"), ("BIGOTIMES", "\\bigotimes"), ("BIGODOT", "\\bigodot"), ("BIGUPLUS", "\\biguplus"),
    ("inter", "\\bigcap"), ("union", "\\bigcup"),
    ("BIGOMINUS", "{\\Large\\ominus}"), ("BIGODIV", "{\\Large\\oslash}"),
    ("UNDEROVER", ""),
    ("SIM", "\\sim"), ("APPROX", "\\approx"), ("SIMEQ", "\\simeq"), ("CONG", "\\cong"),
    ("==", "\\equiv"),
    ("DIAMOND", "\\diamond"), ("FORALL", "\\forall"),
    ("prime", "'"), ("Partial", "\\partial"), ("INF", "\\infty"), ("PROPTO", "\\propto"),
    ("lim", "\\lim"), ("Lim", "\\lim"),
    ("larrow", "\\leftarrow"), ("->", "\\rightarrow"),
    ("uparrow", "\\uparrow"), ("downarrow", "\\downarrow"),
    ("LARROW", "\\Leftarrow"), ("RARROW", "\\Rightarrow"),
    ("UPARROW", "\\Uparrow"), ("DOWNARROW", "\\Downarrow"),
    ("udarrow", "\\updownarrow"),
    ("<->", "\\leftrightarrow"),
    ("UDARROW", "\\Updownarrow"), ("LRARROW", "\\Leftrightarrow"),
    ("NWARROW", "\\nwarrow"), ("SEARROW", "\\searrow"), ("NEARROW", "\\nearrow"), ("SWARROW", "\\swarrow"),
    ("HOOKLEFT", "\\hookleftarrow"), ("HOOKRIGHT", "\\hookrightarrow"),
    ("PVER", "\\|"), ("MAPSTO", "\\mapsto"),
    ("CDOTS", "\\cdots"), ("LDOTS", "\\ldots"), ("VDOTS", "\\vdots"), ("DDOTS", "\\ddots"),
    ("DAGGER", "\\dagger"), ("DDAGGER", "\\ddagger"), ("DOTEQ", "\\doteq"),
    ("image", "\\fallingdotseq"), ("REIMAGE", "\\risingdotseq"),
    ("ASYMP", "\\asymp"), ("ISO", "\\Bumpeq"),
    ("DSUM", "\\dotplus"), ("XOR", "\\veebar"),
    ("TRIANGLE", "\\triangle"), ("NABLA", "\\nabla"),
    ("ANGLE", "\\angle"), ("MSANGLE", "\\measuredangle"), ("SANGLE", "\\sphericalangle"),
    ("VDASH", "\\vdash"), ("DASHV", "\\dashv"),
    ("BOT", "\\bot"), ("TOP", "\\top"), ("MODELS", "\\models"),
    ("LAPLACE", "\\mathcal{L}"),
    ("CENTIGRADE", "^{\\circ}C"), ("FAHRENHEIT", "^{\\circ}F"),
    ("LSLANT", "\\diagup"), ("RSLANT", "\\diagdown"),

    ("sqrt", "\\sqrt"),
    ("int", "\\int"), ("dint", "\\iint"), ("tint", "\\iiint"), ("oint", "\\oint"),

    ("alpha", "\\alpha"), ("beta", "\\beta"), ("gamma", "\\gamma"), ("delta", "\\delta"),
    ("epsilon", "\\epsilon"), ("zeta", "\\zeta"), ("eta", "\\eta"), ("theta", "\\theta"),
    ("iota", "\\iota"), ("kappa", "\\kappa"), ("lambda", "\\lambda"), ("mu", "\\mu"),
    ("nu", "\\nu"), ("xi", "\\xi"), ("omicron", "\\omicron"), ("pi", "\\pi"),
    ("rho", "\\rho"), ("sigma", "\\sigma"), ("tau", "\\tau"), ("upsilon", "\\upsilon"),
    ("phi", "\\phi"), ("chi", "\\chi"), ("psi", "\\psi"), ("omega", "\\omega"),
    ("ALPHA", "A"), ("BETA", "B"), ("GAMMA", "\\Gamma"), ("DELTA", "\\Delta"),
    ("EPSILON", "E"), ("ZETA", "Z"), ("ETA", "H"), ("THETA", "\\Theta"),
    ("IOTA", "I"), ("KAPPA", "K"), ("LAMBDA", "\\Lambda"), ("MU", "M"),
    ("NU", "N"), ("XI", "\\Xi"), ("OMICRON", "O"), ("PI", "\\Pi"),
    ("RHO", "P"), ("SIGMA", "\\Sigma"), ("TAU", "T"), ("UPSILON", "\\Upsilon"),
    ("PHI", "\\Phi"), ("CHI", "X"), ("PSI", "\\Psi"), ("OMEGA", "\\Omega"),

    ("\u{2308}", "\\lceil"), ("\u{2309}", "\\rceil"),
    ("\u{230a}", "\\lfloor"), ("\u{230b}", "\\rfloor"),
    ("\u{2225}", "\\|"),
    ("\u{2290}", "\\sqsupset"), ("\u{2292}", "\\sqsupseteq"),

    ("odint", "\\mathop \u{222f}"),
    ("otint", "\\mathop \u{2230}"),
    ("BIGSQCAP", "\\mathop \u{2a45}"),
    ("ATT", "\\mathop \u{203b}"),
    ("HUND", "\\mathop \u{2030}"),
    ("THOU", "\\mathop \u{2031}"),
    ("IDENTICAL", "\\mathop \u{2237}"),
    ("RTANGLE", "\\mathop \u{22be}"),
    ("BASE", "\\mathop \u{2302}"),
    ("BENZENE", "\\mathop \u{232c}"),
];

// Tokens rewritten to a HULK-prefixed marker, then expanded in second passes.
pub static MIDDLE_CONVERT_MAP_ENTRIES: &[(&str, &str)] = &[
    ("matrix", "HULKMATRIX"),
    ("pmatrix", "HULKPMATRIX"),
    ("bmatrix", "HULKBMATRIX"),
    ("dmatrix", "HULKDMATRIX"),
    ("eqalign", "HULKEQALIGN"),
    ("cases", "HULKCASE"),
    ("vec", "HULKVEC"),
    ("dyad", "HULKDYAD"),
    ("acute", "HULKACUTE"),
    ("grave", "HULKGRAVE"),
    ("dot", "HULKDOT"),
    ("ddot", "HULKDDOT"),
    ("bar", "HULKBAR"),
    ("hat", "HULKHAT"),
    ("check", "HULKCHECK"),
    ("arch", "HULKARCH"),
    ("tilde", "HULKTILDE"),
    ("BOX", "HULKBOX"),
    ("OVERBRACE", "HULKOVERBRACE"),
    ("UNDERBRACE", "HULKUNDERBRACE"),
];

pub static BAR_CONVERT_MAP: &[(&str, &str)] = &[
    ("HULKVEC", "\\overrightarrow"),
    ("HULKDYAD", "\\overleftrightarrow"),
    ("HULKACUTE", "\\acute"),
    ("HULKGRAVE", "\\grave"),
    ("HULKDOT", "\\dot"),
    ("HULKDDOT", "\\ddot"),
    ("HULKBAR", "\\overline"),
    ("HULKHAT", "\\widehat"),
    ("HULKCHECK", "\\check"),
    ("HULKARCH", "\\overset{\\frown}"),
    ("HULKTILDE", "\\widetilde"),
    ("HULKBOX", "\\boxed"),
];

pub static MATRIX_CONVERT_MAP: &[(&str, MatrixMapping)] = &[
    ("HULKMATRIX", MatrixMapping { begin: "\\begin{matrix}", end: "\\end{matrix}", remove_outer_brackets: true }),
    ("HULKPMATRIX", MatrixMapping { begin: "\\begin{pmatrix}", end: "\\end{pmatrix}", remove_outer_brackets: true }),
    ("HULKBMATRIX", MatrixMapping { begin: "\\begin{bmatrix}", end: "\\end{bmatrix}", remove_outer_brackets: true }),
    ("HULKDMATRIX", MatrixMapping { begin: "\\begin{vmatrix}", end: "\\end{vmatrix}", remove_outer_brackets: true }),
    ("HULKCASE", MatrixMapping { begin: "\\begin{cases}", end: "\\end{cases}", remove_outer_brackets: true }),
    ("HULKEQALIGN", MatrixMapping { begin: "\\eqalign{", end: "}", remove_outer_brackets: false }),
];

pub static BRACE_CONVERT_MAP: &[(&str, &str)] = &[
    ("HULKOVERBRACE", "\\overbrace"),
    ("HULKUNDERBRACE", "\\underbrace"),
];

lazy_static! {
    pub static ref CONVERT_MAP: HashMap<&'static str, &'static str> =
        CONVERT_MAP_ENTRIES.iter().copied().collect();
    pub static ref MIDDLE_CONVERT_MAP: HashMap<&'static str, &'static str> =
        MIDDLE_CONVERT_MAP_ENTRIES.iter().copied().collect();
}
