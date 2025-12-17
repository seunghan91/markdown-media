# Python 패키지 사용 가이드

MDM Python 패키지는 HWP, PDF, HTML 문서를 처리하고 Markdown+Media 형식으로 변환합니다.

## 설치

### PyPI에서 설치 (권장)

```bash
pip install mdm-parser-py
```

### 소스에서 설치

```bash
cd packages/parser-py
pip install -e .
```

### 종속성 설치

```bash
pip install -r requirements.txt
```

**필수 종속성:**

- `pyhwp` - HWP 파일 처리
- `pdfplumber` - PDF 텍스트/이미지 추출
- `pillow` - 이미지 처리
- `svgwrite` - SVG 생성
- `beautifulsoup4` - HTML 파싱
- `requests` - HTTP 요청

**선택 종속성:**

- `pytesseract` - OCR (Tesseract 엔진)
- `easyocr` - OCR (딥러닝 기반)

---

## 모듈별 사용법

### 1. hwp_to_svg.py - HWP 표를 SVG로 변환

```python
from packages.parser_py.hwp_to_svg import HwpToSvgConverter

# 변환기 생성
converter = HwpToSvgConverter('document.hwp')

# SVG로 변환
svg_files = converter.convert('output_dir/')

# 결과
# output_dir/
#   ├── table_1.svg
#   ├── table_2.svg
#   └── ...
```

**명령줄 사용:**

```bash
python hwp_to_svg.py document.hwp output_dir/
```

---

### 2. pdf_processor.py - PDF 텍스트/이미지 추출

```python
from packages.parser_py.pdf_processor import PdfProcessor

# 프로세서 생성
processor = PdfProcessor('report.pdf')

# 텍스트 추출
text = processor.extract_text()
print(text)

# 이미지 추출
images = processor.extract_images('assets/')
# images = [{'page': 1, 'filename': 'page1_img1.png', 'path': 'assets/page1_img1.png'}, ...]

# 메타데이터 추출
metadata = processor.extract_metadata()
# {'pages': 10, 'metadata': {...}}
```

**명령줄 사용:**

```bash
# 텍스트 추출
python pdf_processor.py document.pdf

# 이미지 추출
python pdf_processor.py document.pdf --extract-images assets/
```

---

### 3. ocr_processor.py - OCR (이미지 → 텍스트)

```python
from packages.parser_py.ocr_processor import OcrProcessor

# OCR 프로세서 생성 (자동 엔진 선택)
processor = OcrProcessor(engine='auto', lang='kor+eng')

# 이미지에서 텍스트 추출
text = processor.extract_text('scanned_document.png')
print(text)

# 디렉토리 일괄 처리
results = processor.process_directory('images/', 'output.txt')
```

**명령줄 사용:**

```bash
# 단일 이미지
python ocr_processor.py image.png

# 디렉토리 처리
python ocr_processor.py --dir images/ output.txt

# 특정 엔진 지정
python ocr_processor.py --engine easyocr image.png
python ocr_processor.py --engine tesseract image.png
```

**지원 엔진:**

- `tesseract` - Tesseract OCR (빠름, 가벼움)
- `easyocr` - EasyOCR (정확도 높음, 딥러닝)
- `auto` - 자동 선택

---

## 코드 예시

### 예시 1: HWP 파일에서 텍스트 추출

```python
from packages.parser_py.hwp_to_svg import HwpToSvgConverter

def extract_hwp_content(hwp_path, output_dir):
    converter = HwpToSvgConverter(hwp_path)

    # 표를 SVG로 변환
    svg_files = converter.convert(output_dir)

    print(f"생성된 SVG 파일: {len(svg_files)}개")
    for svg in svg_files:
        print(f"  - {svg}")

    return svg_files

# 사용
extract_hwp_content('report.hwp', './output/')
```

### 예시 2: PDF 보고서 처리

```python
from packages.parser_py.pdf_processor import PdfProcessor
import json

def process_pdf_report(pdf_path):
    processor = PdfProcessor(pdf_path)

    # 텍스트 추출
    text = processor.extract_text()

    # 메타데이터 추출
    meta = processor.extract_metadata()

    # 이미지 추출
    images = processor.extract_images('./assets/')

    # 결과 저장
    result = {
        'text': text,
        'metadata': meta,
        'images': images
    }

    with open('report_data.json', 'w', encoding='utf-8') as f:
        json.dump(result, f, ensure_ascii=False, indent=2)

    return result

# 사용
process_pdf_report('annual_report.pdf')
```

### 예시 3: 스캔 문서 OCR

```python
from packages.parser_py.ocr_processor import OcrProcessor
import os

def ocr_scanned_documents(input_dir, output_dir):
    processor = OcrProcessor(engine='easyocr', lang='kor+eng')

    os.makedirs(output_dir, exist_ok=True)

    for filename in os.listdir(input_dir):
        if filename.endswith(('.png', '.jpg', '.jpeg')):
            image_path = os.path.join(input_dir, filename)

            # OCR 수행
            text = processor.extract_text(image_path)

            # 결과 저장
            output_file = os.path.join(output_dir, f"{filename}.txt")
            with open(output_file, 'w', encoding='utf-8') as f:
                f.write(text)

            print(f"처리 완료: {filename}")

# 사용
ocr_scanned_documents('./scans/', './ocr_results/')
```

---

## 에러 처리

```python
from packages.parser_py.pdf_processor import PdfProcessor

try:
    processor = PdfProcessor('document.pdf')
    text = processor.extract_text()
except FileNotFoundError:
    print("파일을 찾을 수 없습니다")
except ValueError as e:
    print(f"잘못된 파일 형식: {e}")
except ImportError:
    print("pdfplumber가 설치되지 않았습니다")
    print("설치: pip install pdfplumber")
```

---

## API 참조

### HwpToSvgConverter

| 메서드                                         | 설명                                     |
| ---------------------------------------------- | ---------------------------------------- |
| `__init__(file_path)`                          | HWP 파일 경로로 초기화                   |
| `convert(output_dir)`                          | 표를 SVG로 변환, 생성된 파일 리스트 반환 |
| `extract_tables(hwp)`                          | pyhwp 객체에서 표 데이터 추출            |
| `render_table_to_svg(table_data, output_path)` | 표 데이터를 SVG로 렌더링                 |

### PdfProcessor

| 메서드                       | 설명                   |
| ---------------------------- | ---------------------- |
| `__init__(file_path)`        | PDF 파일 경로로 초기화 |
| `extract_text()`             | 전체 텍스트 추출       |
| `extract_images(output_dir)` | 이미지 추출 및 저장    |
| `extract_metadata()`         | 메타데이터 추출        |

### OcrProcessor

| 메서드                                      | 설명                   |
| ------------------------------------------- | ---------------------- |
| `__init__(engine, lang)`                    | OCR 엔진과 언어 설정   |
| `extract_text(image_path)`                  | 이미지에서 텍스트 추출 |
| `process_directory(input_dir, output_file)` | 디렉토리 일괄 처리     |

---

## 문제 해결

### pyhwp 설치 오류

```bash
# 시스템 패키지 설치 (Ubuntu)
sudo apt-get install libxml2-dev libxslt1-dev

# pip 재설치
pip install --upgrade pyhwp
```

### Tesseract 설치

```bash
# macOS
brew install tesseract tesseract-lang

# Ubuntu
sudo apt-get install tesseract-ocr tesseract-ocr-kor

# Python 바인딩
pip install pytesseract
```

### EasyOCR GPU 지원

```bash
# CUDA 있는 경우
pip install easyocr torch torchvision
```

---

**Author**: seunghan91
**Version**: 0.1.0
