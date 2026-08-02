"""Text and structure similarity metrics for bench runs.

Lazy imports: metric libraries are optional; enable via config.toml.
"""
from __future__ import annotations

from typing import Protocol


class Metric(Protocol):
    name: str
    def score(self, hypothesis: str, reference: str) -> float: ...


_MAX_CMP_CHARS = 200_000  # cap comparison length — BLEU/ROUGE blow up on large docs


def _normalize(s: str) -> str:
    """Strip markdown noise that doesn't affect semantic comparison.
    Caps length at _MAX_CMP_CHARS to keep BLEU/ROUGE tractable on large docs."""
    import re
    s = _normalize_image_refs(s)
    s = re.sub(r"\n{3,}", "\n\n", s)
    s = re.sub(r"[ \t]+", " ", s)
    s = s.strip()
    return s[:_MAX_CMP_CHARS]


def _normalize_image_refs(s: str) -> str:
    """Reduce an image reference to `<IMG:alt>` on both sides of a comparison.

    The ground truth still carries the pre-P0 `@[[image_N]]` syntax while the
    converter emits CommonMark `![image_N](assets/images/<hash>.png)`. Scoring
    them as-is punishes image-heavy fixtures for a syntax migration rather than
    for output quality — measured: fixtures with 169-208 references score
    cer 0.42-0.67, those with none score 0.00-0.03.

    The alt text is kept deliberately. Collapsing every reference to one opaque
    token would also hide a wrong *number*, *order* or *label* of images, which
    is real output quality. Only the path is dropped, because the hash filename
    is an implementation detail the hand-curated reference must not be pinned
    to; whether those paths actually resolve is checked by
    `scripts/verify_bundle.py --strict`, not here.

    Inline links (`[text](url)`) are left alone — only the `!`-prefixed image
    form and the legacy form are rewritten.
    """
    import re
    s = re.sub(r"!\[([^\]]*)\]\([^)]*\)", lambda m: f"<IMG:{m.group(1).strip()}>", s)
    s = re.sub(r"@\[\[([^\]]+)\]\]", lambda m: f"<IMG:{m.group(1).strip()}>", s)
    return s


class BleuMetric:
    name = "bleu"
    def score(self, hyp: str, ref: str) -> float:
        from nltk.translate.bleu_score import sentence_bleu, SmoothingFunction
        h = _normalize(hyp).split()
        r = _normalize(ref).split()
        if not h or not r:
            return 0.0
        return sentence_bleu([r], h, smoothing_function=SmoothingFunction().method1)


class RougeLMetric:
    name = "rouge_l"
    def score(self, hyp: str, ref: str) -> float:
        from rouge_score import rouge_scorer
        scorer = rouge_scorer.RougeScorer(["rougeL"], use_stemmer=False)
        return scorer.score(_normalize(ref), _normalize(hyp))["rougeL"].fmeasure


class CerMetric:
    name = "cer"
    def score(self, hyp: str, ref: str) -> float:
        import jiwer
        return jiwer.cer(_normalize(ref), _normalize(hyp))


class EditRatioMetric:
    name = "edit_ratio"
    def score(self, hyp: str, ref: str) -> float:
        import Levenshtein
        return Levenshtein.ratio(_normalize(hyp), _normalize(ref))


class _MdNode:
    """Minimal tree node consumed by APTED — wraps a mistune AST token.
    Label is the token type (heading/paragraph/list/...); children are nested tokens.
    Text content is preserved as a synthetic child so text differences show up."""
    __slots__ = ("name", "children")
    def __init__(self, name: str, children: list["_MdNode"] | None = None):
        self.name = name
        self.children = children or []


def _make_md_config():
    """Lazy import so metrics module is importable without apted installed."""
    from apted import Config as _Base
    class _MdNodeConfig(_Base):
        def rename(self, n1, n2):
            return 0 if n1.name == n2.name else 1
        def children(self, node):
            return node.children
    return _MdNodeConfig()


def _ast_to_tree(tokens) -> list[_MdNode]:
    out: list[_MdNode] = []
    if not tokens:
        return out
    for tok in tokens:
        if not isinstance(tok, dict):
            continue
        ttype = tok.get("type", "?")
        if ttype == "blank_line":
            continue
        kids = _ast_to_tree(tok.get("children", []))
        # Collapse long text into a single leaf; label by first 20 chars
        # so purely-textual changes still affect the edit distance without
        # blowing up the tree size on large documents.
        raw = tok.get("raw")
        if raw and ttype in {"text", "codespan"}:
            label = f"{ttype}:{raw[:20]}"
            out.append(_MdNode(label))
        else:
            out.append(_MdNode(ttype, kids))
    return out


class TsedMetric:
    """Markdown AST tree-edit similarity using APTED.
    Returns 1 - editdist / max(|hyp_tree|, |ref_tree|), in [0, 1]."""
    name = "tsed"
    def score(self, hyp: str, ref: str) -> float:
        try:
            from apted import APTED
            import mistune
        except ImportError:
            return 0.0
        md = mistune.create_markdown(renderer=None)
        hyp_ast = md(_normalize(hyp)) or []
        ref_ast = md(_normalize(ref)) or []
        hyp_nodes = _ast_to_tree(hyp_ast)
        ref_nodes = _ast_to_tree(ref_ast)
        if not hyp_nodes and not ref_nodes:
            return 1.0
        # Wrap in a root so APTED sees a single tree
        hyp_root = _MdNode("root", hyp_nodes)
        ref_root = _MdNode("root", ref_nodes)

        def tree_size(n: _MdNode) -> int:
            return 1 + sum(tree_size(c) for c in n.children)

        a = APTED(hyp_root, ref_root, _make_md_config())
        dist = a.compute_edit_distance()
        size_max = max(tree_size(hyp_root), tree_size(ref_root))
        if size_max == 0:
            return 1.0
        return max(0.0, 1.0 - dist / size_max)


REGISTRY: dict[str, Metric] = {
    m.name: m() for m in (BleuMetric, RougeLMetric, CerMetric, EditRatioMetric, TsedMetric)
}


def score_all(hyp: str, ref: str, enabled: list[str]) -> dict[str, float]:
    out = {}
    for name in enabled:
        m = REGISTRY.get(name)
        if m is None:
            continue
        try:
            out[name] = m.score(hyp, ref)
        except Exception as e:
            out[name] = float("nan")
            out[f"{name}_error"] = str(e)
    return out
