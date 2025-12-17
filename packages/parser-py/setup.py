from setuptools import setup, find_packages

setup(
    name="mdm-parser-py",
    version="0.1.0",
    packages=find_packages(),
    install_requires=[
        "pyhwp",
        "pdfplumber",
        "pillow",
        "svgwrite",
    ],
    author="seunghan91",
    description="Python helper for MDM parser",
    python_requires=">=3.8",
)
