#!/usr/bin/env python3
"""
OCR Bridge - Bridges Rust parser output with OCR processing
Integrates mdm-core Rust output with Python OCR for image-embedded text extraction

Supports multiple OCR engines:
- Tesseract (local, offline)
- EasyOCR (local, offline)
- OpenRouter AI (cloud, requires API key) - Uses vision models like Qwen3-VL, Yi-Vision
"""
import os
import sys
import json
import re
import tempfile
import base64
import mimetypes
from pathlib import Path
from typing import Dict, List, Optional, Any, Tuple
from dataclasses import dataclass, field

# Import local OCR processor
try:
    from .ocr_processor import OcrProcessor
except ImportError:
    from ocr_processor import OcrProcessor

# OpenRouter API configuration
OPENROUTER_API_URL = "https://openrouter.ai/api/v1/chat/completions"
OPENROUTER_FREE_MODELS = [
    "amazon/nova-lite-v1:free",
    "qwen/qwen-2.5-vl-72b-instruct:free",
    "meta-llama/llama-4-maverick:free",
]
OPENROUTER_PAID_MODELS = [
    "qwen/qwen-3-vl-32b-instruct",  # Best for multilingual OCR (32 languages)
    "anthropic/claude-sonnet-4",    # High quality vision
    "google/gemini-2.0-flash-001",  # Fast and accurate
    "mistral/mistral-small-3.1-24b-instruct",  # Good for documents
]


class OpenRouterOcrEngine:
    """
    AI-based OCR using OpenRouter's vision models.

    Supports both free and paid models. Users can provide their own API key
    for unlimited access to premium models with better accuracy.
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        use_free_model: bool = True,
        custom_prompt: Optional[str] = None,
    ):
        """
        Initialize OpenRouter OCR engine.

        Args:
            api_key: OpenRouter API key (or set OPENROUTER_API_KEY env var)
            model: Specific model to use (defaults to free model if use_free_model=True)
            use_free_model: Use free-tier model (default: True)
            custom_prompt: Custom extraction prompt
        """
        self.api_key = api_key or os.environ.get("OPENROUTER_API_KEY", "")

        if not self.api_key:
            raise ValueError(
                "OpenRouter API key required. Set OPENROUTER_API_KEY environment variable "
                "or pass api_key parameter. Get your key at: https://openrouter.ai/keys"
            )

        if model:
            self.model = model
        elif use_free_model:
            self.model = OPENROUTER_FREE_MODELS[0]
        else:
            self.model = OPENROUTER_PAID_MODELS[0]

        self.custom_prompt = custom_prompt or self._default_prompt()

        # Check for requests library
        try:
            import requests
            self.requests = requests
        except ImportError:
            raise ImportError("requests library required: pip install requests")

    def _default_prompt(self) -> str:
        """Default OCR extraction prompt"""
        return """Extract ALL text from this image exactly as it appears.
Preserve the original:
- Reading order (top to bottom, left to right)
- Line breaks and paragraph structure
- Any formatting (headers, lists, tables)
- Numbers, dates, and special characters

For tables, represent as markdown format.
For Korean/Asian text, maintain proper character encoding.

Output ONLY the extracted text, no explanations or metadata."""

    def _encode_image(self, image_path: str) -> Tuple[str, str]:
        """Encode image to base64 with proper MIME type"""
        mime_type, _ = mimetypes.guess_type(image_path)
        if mime_type is None:
            # Default based on extension
            ext = Path(image_path).suffix.lower()
            mime_map = {
                ".jpg": "image/jpeg",
                ".jpeg": "image/jpeg",
                ".png": "image/png",
                ".gif": "image/gif",
                ".webp": "image/webp",
                ".bmp": "image/bmp",
            }
            mime_type = mime_map.get(ext, "image/png")

        with open(image_path, "rb") as f:
            image_data = base64.b64encode(f.read()).decode("utf-8")

        return f"data:{mime_type};base64,{image_data}", mime_type

    def extract_text(self, image_path: str, prompt: Optional[str] = None) -> str:
        """
        Extract text from image using OpenRouter vision model.

        Args:
            image_path: Path to image file
            prompt: Optional custom prompt for this extraction

        Returns:
            Extracted text
        """
        if not os.path.exists(image_path):
            raise FileNotFoundError(f"Image not found: {image_path}")

        image_data, mime_type = self._encode_image(image_path)
        extraction_prompt = prompt or self.custom_prompt

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
            "HTTP-Referer": "https://github.com/seunghan91/markdown-media",
            "X-Title": "MDM Parser OCR",
        }

        payload = {
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": extraction_prompt,
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_data,
                            },
                        },
                    ],
                }
            ],
            "max_tokens": 4096,
            "temperature": 0.1,  # Low temperature for accurate extraction
        }

        try:
            response = self.requests.post(
                OPENROUTER_API_URL,
                headers=headers,
                json=payload,
                timeout=60,
            )
            response.raise_for_status()

            result = response.json()

            if "choices" in result and len(result["choices"]) > 0:
                return result["choices"][0]["message"]["content"].strip()
            else:
                raise ValueError(f"Unexpected API response: {result}")

        except self.requests.exceptions.RequestException as e:
            raise RuntimeError(f"OpenRouter API request failed: {e}")

    def extract_text_from_url(self, image_url: str, prompt: Optional[str] = None) -> str:
        """
        Extract text from image URL using OpenRouter vision model.

        Args:
            image_url: URL of the image
            prompt: Optional custom prompt

        Returns:
            Extracted text
        """
        extraction_prompt = prompt or self.custom_prompt

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
            "HTTP-Referer": "https://github.com/seunghan91/markdown-media",
            "X-Title": "MDM Parser OCR",
        }

        payload = {
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": extraction_prompt,
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url,
                            },
                        },
                    ],
                }
            ],
            "max_tokens": 4096,
            "temperature": 0.1,
        }

        try:
            response = self.requests.post(
                OPENROUTER_API_URL,
                headers=headers,
                json=payload,
                timeout=60,
            )
            response.raise_for_status()

            result = response.json()

            if "choices" in result and len(result["choices"]) > 0:
                return result["choices"][0]["message"]["content"].strip()
            else:
                raise ValueError(f"Unexpected API response: {result}")

        except self.requests.exceptions.RequestException as e:
            raise RuntimeError(f"OpenRouter API request failed: {e}")

    @staticmethod
    def available_models() -> Dict[str, List[str]]:
        """List available models by category"""
        return {
            "free": OPENROUTER_FREE_MODELS,
            "paid": OPENROUTER_PAID_MODELS,
        }


@dataclass
class OcrResult:
    """OCR result for a single image"""
    image_id: str
    source_path: str
    extracted_text: str
    confidence: float = 0.0
    language: str = "unknown"
    bounding_boxes: List[Dict[str, Any]] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "image_id": self.image_id,
            "source_path": self.source_path,
            "extracted_text": self.extracted_text,
            "confidence": self.confidence,
            "language": self.language,
            "bounding_boxes": self.bounding_boxes,
            "metadata": self.metadata,
        }


@dataclass
class RustOutput:
    """Parsed output from Rust mdm-core parser"""
    format: str  # hwp, hwpx, pdf, docx
    version: str
    metadata: Dict[str, Any]
    text_content: str
    images: List[Dict[str, Any]]
    tables: List[Dict[str, Any]]
    output_dir: Optional[str] = None

    @classmethod
    def from_json(cls, json_path: str) -> "RustOutput":
        """Load from JSON output file"""
        with open(json_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        return cls(
            format=data.get("format", "unknown"),
            version=data.get("version", ""),
            metadata=data.get("metadata", {}),
            text_content=data.get("text", ""),
            images=data.get("images", []),
            tables=data.get("tables", []),
            output_dir=str(Path(json_path).parent),
        )

    @classmethod
    def from_mdx(cls, mdx_path: str) -> "RustOutput":
        """Parse from MDX output file with frontmatter"""
        with open(mdx_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Parse YAML frontmatter
        metadata = {}
        text_content = content

        if content.startswith("---"):
            parts = content.split("---", 2)
            if len(parts) >= 3:
                # Parse frontmatter
                frontmatter = parts[1].strip()
                for line in frontmatter.split("\n"):
                    if ":" in line:
                        key, value = line.split(":", 1)
                        key = key.strip()
                        value = value.strip().strip('"')
                        # Convert numeric values
                        if value.isdigit():
                            value = int(value)
                        metadata[key] = value
                text_content = parts[2].strip()

        # Extract image references
        images = []
        image_pattern = r'!\[(.*?)\]\((.*?)\)'
        for match in re.finditer(image_pattern, text_content):
            alt_text, path = match.groups()
            images.append({
                "id": Path(path).stem,
                "filename": Path(path).name,
                "path": path,
                "alt_text": alt_text,
            })

        return cls(
            format=metadata.get("format", "unknown"),
            version=str(metadata.get("version", "")),
            metadata=metadata,
            text_content=text_content,
            images=images,
            tables=[],
            output_dir=str(Path(mdx_path).parent),
        )


class RustOcrBridge:
    """
    Bridge between Rust mdm-core parser output and Python OCR processing.

    This class processes the output from the Rust parser, identifies images
    that need OCR, performs text extraction, and enhances the MDX output
    with the extracted text.

    Supports multiple OCR engines:
    - 'tesseract': Local Tesseract OCR (requires pytesseract)
    - 'easyocr': Local EasyOCR (requires easyocr)
    - 'openrouter': Cloud AI OCR via OpenRouter (requires API key)
    - 'auto': Auto-select best available local engine
    """

    def __init__(
        self,
        ocr_engine: str = "auto",
        language: str = "kor+eng",
        min_image_size: Tuple[int, int] = (50, 50),
        confidence_threshold: float = 0.5,
        openrouter_api_key: Optional[str] = None,
        openrouter_model: Optional[str] = None,
        use_free_model: bool = True,
    ):
        """
        Initialize the OCR bridge.

        Args:
            ocr_engine: OCR engine to use:
                - 'tesseract': Local Tesseract OCR
                - 'easyocr': Local EasyOCR
                - 'openrouter': Cloud AI OCR via OpenRouter
                - 'auto': Auto-select best available local engine
            language: Language code for OCR (default: Korean + English)
            min_image_size: Minimum image dimensions to process
            confidence_threshold: Minimum confidence to include results
            openrouter_api_key: API key for OpenRouter (or set OPENROUTER_API_KEY env var)
            openrouter_model: Specific OpenRouter model to use
            use_free_model: Use free-tier OpenRouter model (default: True)
        """
        self.engine_type = ocr_engine.lower()
        self.language = language
        self.min_image_size = min_image_size
        self.confidence_threshold = confidence_threshold
        self._ocr_cache: Dict[str, OcrResult] = {}

        # Initialize appropriate OCR engine
        if self.engine_type == "openrouter":
            self.ocr_processor = None
            self.ai_engine = OpenRouterOcrEngine(
                api_key=openrouter_api_key,
                model=openrouter_model,
                use_free_model=use_free_model,
            )
        else:
            self.ocr_processor = OcrProcessor(engine=ocr_engine, lang=language)
            self.ai_engine = None

    def process_rust_output(self, rust_output_path: str) -> Dict[str, Any]:
        """
        Process output from Rust mdm-core parser.

        Args:
            rust_output_path: Path to Rust output (JSON or MDX file)

        Returns:
            Dict containing:
                - source: Original parsed document info
                - ocr_results: List of OCR results for images
                - enhanced_text: Text with OCR content integrated
                - statistics: Processing statistics
        """
        # Load Rust output
        if rust_output_path.endswith(".json"):
            rust_output = RustOutput.from_json(rust_output_path)
        elif rust_output_path.endswith((".mdx", ".md")):
            rust_output = RustOutput.from_mdx(rust_output_path)
        else:
            raise ValueError(f"Unsupported output format: {rust_output_path}")

        # Process images with OCR
        ocr_results = []
        processed_count = 0
        skipped_count = 0
        error_count = 0

        for image_info in rust_output.images:
            image_path = self._resolve_image_path(image_info, rust_output.output_dir)

            if not image_path or not os.path.exists(image_path):
                skipped_count += 1
                continue

            try:
                result = self._process_image(image_info, image_path)
                if result:
                    ocr_results.append(result)
                    processed_count += 1
                else:
                    skipped_count += 1
            except Exception as e:
                error_count += 1
                print(f"Warning: Failed to process {image_info.get('id', 'unknown')}: {e}")

        # Build enhanced text
        enhanced_text = self._integrate_ocr_results(rust_output.text_content, ocr_results)

        return {
            "source": {
                "format": rust_output.format,
                "version": rust_output.version,
                "metadata": rust_output.metadata,
                "image_count": len(rust_output.images),
                "table_count": len(rust_output.tables),
            },
            "ocr_results": [r.to_dict() for r in ocr_results],
            "enhanced_text": enhanced_text,
            "statistics": {
                "total_images": len(rust_output.images),
                "processed": processed_count,
                "skipped": skipped_count,
                "errors": error_count,
                "ocr_engine": self.ocr_processor.engine,
            },
        }

    def _resolve_image_path(
        self, image_info: Dict[str, Any], output_dir: Optional[str]
    ) -> Optional[str]:
        """Resolve actual image file path from image info"""
        # Try direct path
        path = image_info.get("path", "")
        if path and os.path.isabs(path) and os.path.exists(path):
            return path

        # Try relative to output directory
        if output_dir:
            filename = image_info.get("filename", "") or image_info.get("id", "")
            if filename:
                for ext in ["", ".png", ".jpg", ".jpeg", ".gif", ".bmp"]:
                    full_path = os.path.join(output_dir, filename + ext)
                    if os.path.exists(full_path):
                        return full_path

                # Check media subdirectory
                media_path = os.path.join(output_dir, "media", filename)
                for ext in ["", ".png", ".jpg", ".jpeg", ".gif", ".bmp"]:
                    full_path = media_path + ext
                    if os.path.exists(full_path):
                        return full_path

        # Try embedded base64 data
        if "data" in image_info and image_info["data"]:
            return self._save_temp_image(image_info)

        return None

    def _save_temp_image(self, image_info: Dict[str, Any]) -> Optional[str]:
        """Save base64 encoded image data to temp file"""
        data = image_info.get("data", "")
        if not data:
            return None

        # Handle base64 data
        if isinstance(data, str):
            # Remove data URL prefix if present
            if data.startswith("data:"):
                data = data.split(",", 1)[1]
            image_bytes = base64.b64decode(data)
        elif isinstance(data, (bytes, bytearray)):
            image_bytes = bytes(data)
        else:
            return None

        # Determine extension
        ext = ".png"
        fmt = image_info.get("format", "").lower()
        if fmt in ["jpg", "jpeg"]:
            ext = ".jpg"
        elif fmt == "gif":
            ext = ".gif"

        # Save to temp file
        temp_file = tempfile.NamedTemporaryFile(
            suffix=ext, delete=False, prefix=f"mdm_ocr_{image_info.get('id', 'img')}_"
        )
        temp_file.write(image_bytes)
        temp_file.close()
        return temp_file.name

    def _process_image(
        self, image_info: Dict[str, Any], image_path: str
    ) -> Optional[OcrResult]:
        """Process a single image with OCR"""
        # Check cache
        cache_key = image_path
        if cache_key in self._ocr_cache:
            return self._ocr_cache[cache_key]

        # Skip small images
        try:
            from PIL import Image
            with Image.open(image_path) as img:
                if img.width < self.min_image_size[0] or img.height < self.min_image_size[1]:
                    return None
        except Exception:
            pass  # Continue anyway if PIL fails

        # Run OCR using appropriate engine
        if self.ai_engine:
            # Use OpenRouter AI engine
            extracted_text = self.ai_engine.extract_text(image_path)
            engine_name = f"openrouter:{self.ai_engine.model}"
        else:
            # Use local OCR processor
            extracted_text = self.ocr_processor.extract_text(image_path)
            engine_name = self.ocr_processor.engine

        if not extracted_text or len(extracted_text.strip()) < 2:
            return None

        result = OcrResult(
            image_id=image_info.get("id", Path(image_path).stem),
            source_path=image_path,
            extracted_text=extracted_text.strip(),
            language=self.language.split("+")[0],  # Primary language
            metadata={
                "width": image_info.get("width"),
                "height": image_info.get("height"),
                "format": image_info.get("format"),
                "ocr_engine": engine_name,
            },
        )

        # Cache result
        self._ocr_cache[cache_key] = result
        return result

    def _integrate_ocr_results(
        self, text_content: str, ocr_results: List[OcrResult]
    ) -> str:
        """Integrate OCR results into the text content"""
        if not ocr_results:
            return text_content

        # Build OCR content section
        ocr_section = "\n\n## OCR Extracted Text\n\n"
        ocr_section += "<details>\n<summary>Extracted text from images</summary>\n\n"

        for result in ocr_results:
            ocr_section += f"### {result.image_id}\n\n"
            ocr_section += f"```text\n{result.extracted_text}\n```\n\n"

        ocr_section += "</details>\n"

        # Also try to insert OCR text near image references
        enhanced_content = text_content
        for result in ocr_results:
            # Find image references and add OCR text
            patterns = [
                rf'!\[([^\]]*)\]\([^)]*{re.escape(result.image_id)}[^)]*\)',
                rf'<img[^>]*{re.escape(result.image_id)}[^>]*>',
            ]

            for pattern in patterns:
                matches = list(re.finditer(pattern, enhanced_content))
                for match in reversed(matches):  # Reverse to preserve positions
                    insert_pos = match.end()
                    ocr_inline = f"\n\n> **OCR:** {result.extracted_text[:200]}{'...' if len(result.extracted_text) > 200 else ''}\n"
                    enhanced_content = (
                        enhanced_content[:insert_pos]
                        + ocr_inline
                        + enhanced_content[insert_pos:]
                    )
                    break  # Only insert once per result

        # Append full OCR section at the end
        enhanced_content += ocr_section

        return enhanced_content

    def enhance_mdx_with_ocr(
        self,
        mdx_path: str,
        ocr_results: Optional[Dict[str, Any]] = None,
        output_path: Optional[str] = None,
    ) -> str:
        """
        Enhance MDX file with OCR results.

        Args:
            mdx_path: Path to MDX file
            ocr_results: Pre-computed OCR results (or None to compute)
            output_path: Optional output path (defaults to original with .ocr.mdx suffix)

        Returns:
            Path to enhanced MDX file
        """
        # Process if no results provided
        if ocr_results is None:
            ocr_results = self.process_rust_output(mdx_path)

        enhanced_text = ocr_results["enhanced_text"]

        # Preserve frontmatter
        with open(mdx_path, "r", encoding="utf-8") as f:
            original = f.read()

        if original.startswith("---"):
            parts = original.split("---", 2)
            if len(parts) >= 3:
                frontmatter = f"---{parts[1]}---\n\n"
                enhanced_text = frontmatter + enhanced_text

        # Determine output path
        if output_path is None:
            base = Path(mdx_path)
            output_path = str(base.parent / f"{base.stem}.ocr{base.suffix}")

        # Write enhanced MDX
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(enhanced_text)

        return output_path

    def process_directory(
        self,
        input_dir: str,
        output_dir: Optional[str] = None,
        patterns: List[str] = None,
    ) -> Dict[str, Any]:
        """
        Process all MDX files in a directory.

        Args:
            input_dir: Directory containing MDX files
            output_dir: Output directory (defaults to input_dir)
            patterns: File patterns to match (defaults to ["*.mdx", "*.md"])

        Returns:
            Summary of processing results
        """
        if patterns is None:
            patterns = ["*.mdx", "*.md"]

        if output_dir is None:
            output_dir = input_dir

        input_path = Path(input_dir)
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)

        results = {
            "processed": [],
            "skipped": [],
            "errors": [],
        }

        for pattern in patterns:
            for mdx_file in input_path.glob(pattern):
                # Skip already processed files
                if ".ocr." in mdx_file.name:
                    results["skipped"].append(str(mdx_file))
                    continue

                try:
                    print(f"Processing: {mdx_file.name}")
                    output_file = output_path / f"{mdx_file.stem}.ocr{mdx_file.suffix}"

                    ocr_results = self.process_rust_output(str(mdx_file))
                    self.enhance_mdx_with_ocr(str(mdx_file), ocr_results, str(output_file))

                    results["processed"].append({
                        "input": str(mdx_file),
                        "output": str(output_file),
                        "images_processed": ocr_results["statistics"]["processed"],
                    })
                    print(f"  ✓ Enhanced with {ocr_results['statistics']['processed']} OCR results")
                except Exception as e:
                    results["errors"].append({
                        "file": str(mdx_file),
                        "error": str(e),
                    })
                    print(f"  ✗ Error: {e}")

        return results


def main():
    """CLI entry point"""
    import argparse

    parser = argparse.ArgumentParser(
        description="OCR Bridge - Process Rust parser output with OCR",
        epilog="""
Examples:
  # Use local Tesseract OCR
  python ocr_bridge.py document.mdx --engine tesseract

  # Use OpenRouter AI (requires API key)
  export OPENROUTER_API_KEY=sk-or-xxx
  python ocr_bridge.py document.mdx --engine openrouter

  # Use free OpenRouter model
  python ocr_bridge.py document.mdx --engine openrouter --free-model

  # Use specific OpenRouter model
  python ocr_bridge.py document.mdx --engine openrouter --model qwen/qwen-3-vl-32b-instruct

  # List available OpenRouter models
  python ocr_bridge.py --list-models
        """,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "input",
        nargs="?",
        help="Input file (JSON or MDX) or directory",
    )
    parser.add_argument(
        "-o", "--output",
        help="Output file or directory",
    )
    parser.add_argument(
        "--engine",
        choices=["auto", "tesseract", "easyocr", "openrouter"],
        default="auto",
        help="OCR engine to use (default: auto)",
    )
    parser.add_argument(
        "--lang",
        default="kor+eng",
        help="Language code for local OCR (default: kor+eng)",
    )
    parser.add_argument(
        "--api-key",
        help="OpenRouter API key (or set OPENROUTER_API_KEY env var)",
    )
    parser.add_argument(
        "--model",
        help="OpenRouter model to use (see --list-models)",
    )
    parser.add_argument(
        "--free-model",
        action="store_true",
        help="Use free-tier OpenRouter model",
    )
    parser.add_argument(
        "--list-models",
        action="store_true",
        help="List available OpenRouter models",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output results as JSON",
    )
    parser.add_argument(
        "--dir",
        action="store_true",
        help="Process entire directory",
    )

    args = parser.parse_args()

    # Handle list-models command
    if args.list_models:
        models = OpenRouterOcrEngine.available_models()
        print("Available OpenRouter Vision Models:\n")
        print("FREE MODELS (no cost):")
        for model in models["free"]:
            print(f"  - {model}")
        print("\nPAID MODELS (requires credits):")
        for model in models["paid"]:
            print(f"  - {model}")
        print("\nUsage: --engine openrouter --model <model-name>")
        print("Get API key at: https://openrouter.ai/keys")
        sys.exit(0)

    if not args.input:
        parser.error("Input file or directory is required (or use --list-models)")

    try:
        bridge = RustOcrBridge(
            ocr_engine=args.engine,
            language=args.lang,
            openrouter_api_key=args.api_key,
            openrouter_model=args.model,
            use_free_model=args.free_model,
        )

        if args.dir or os.path.isdir(args.input):
            results = bridge.process_directory(args.input, args.output)
            if args.json:
                print(json.dumps(results, indent=2, ensure_ascii=False))
            else:
                print(f"\n✅ Processed {len(results['processed'])} files")
                if results["errors"]:
                    print(f"⚠️  {len(results['errors'])} errors")
        else:
            results = bridge.process_rust_output(args.input)

            if args.json:
                print(json.dumps(results, indent=2, ensure_ascii=False))
            else:
                # Enhance and save
                if args.output:
                    output_path = args.output
                else:
                    base = Path(args.input)
                    output_path = str(base.parent / f"{base.stem}.ocr{base.suffix}")

                bridge.enhance_mdx_with_ocr(args.input, results, output_path)

                print(f"✅ Enhanced MDX saved to: {output_path}")
                stats = results["statistics"]
                print(f"   Images processed: {stats['processed']}/{stats['total_images']}")
                print(f"   OCR engine: {stats['ocr_engine']}")

    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
