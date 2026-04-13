"""MDM Core — HWP/HWPX/PDF/DOCX to Markdown converter (Rust-powered)."""

from mdm_core._mdm_native import (
    convert_file,
    convert_bytes,
    convert_file_to_json,
    detect_format,
    version,
)

__version__ = version()

def convert(path_or_bytes, filename=None):
    """Convert a document to Markdown.

    Args:
        path_or_bytes: File path (str) or bytes data
        filename: Required when passing bytes (for format detection)

    Returns:
        Markdown string

    Examples:
        >>> import mdm_core
        >>> md = mdm_core.convert("document.hwp")
        >>> md = mdm_core.convert(open("doc.pdf", "rb").read(), "doc.pdf")
    """
    if isinstance(path_or_bytes, (bytes, bytearray)):
        if filename is None:
            raise ValueError("filename required when converting bytes")
        return convert_bytes(path_or_bytes, filename)
    return convert_file(str(path_or_bytes))
