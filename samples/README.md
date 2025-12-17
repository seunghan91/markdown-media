# Sample Data

## Input Files

Place sample HWP and PDF files here for testing.

### Recommended Test Files

- `sample.hwp` - Simple HWP document with text
- `sample_table.hwp` - HWP with tables
- `sample.pdf` - Simple PDF document
- `sample_complex.pdf` - PDF with images

## Output

Converted MDX files will be generated in the `output/` directory.

## Usage

```bash
# Convert HWP file
mdm convert samples/input/sample.hwp -o samples/output/

# Convert PDF file
mdm convert samples/input/sample.pdf -o samples/output/
```
