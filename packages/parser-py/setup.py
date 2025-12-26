#!/usr/bin/env python3
"""
MDM Parser Python Package Setup
"""
from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

setup(
    name="mdm-parser",
    version="0.1.0",
    author="seunghan91",
    author_email="seunghan91@users.noreply.github.com",
    description="Python helpers for MDM (Markdown+Media) document conversion",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/seunghan91/markdown-media",
    packages=find_packages(exclude=["tests", "tests.*"]),
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: Text Processing :: Markup",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
    python_requires=">=3.8",
    install_requires=[
        "pdfplumber>=0.10.0",
        "pillow>=9.0.0",
        "svgwrite>=1.4.0",
    ],
    extras_require={
        "hwp": ["pyhwp>=0.1b12"],
        "ocr": ["pytesseract>=0.3.10", "easyocr>=1.7.0"],
        "all": [
            "pyhwp>=0.1b12",
            "pytesseract>=0.3.10",
            "easyocr>=1.7.0",
        ],
    },
    entry_points={
        "console_scripts": [
            "mdm-pdf=pdf_processor:main",
            "mdm-hwp-svg=hwp_to_svg:main",
            "mdm-ocr=ocr_processor:main",
        ],
    },
    keywords="markdown, media, hwp, pdf, document, conversion, mdm",
    project_urls={
        "Bug Reports": "https://github.com/seunghan91/markdown-media/issues",
        "Source": "https://github.com/seunghan91/markdown-media",
        "Documentation": "https://github.com/seunghan91/markdown-media#readme",
    },
)
