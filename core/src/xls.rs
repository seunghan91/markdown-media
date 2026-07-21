//! XLS (Excel 97-2003, BIFF8) spreadsheet parser.
//!
//! Converts legacy `.xls` OLE2/CFB compound files to Markdown pipe tables
//! using `calamine`'s dedicated BIFF8 reader (`calamine::Xls`). The output
//! shape mirrors `crate::xlsx::XlsxDocument` (same sheet-heading + pipe-table
//! rendering, same cell-to-string conversion, same trailing-empty trimming)
//! so downstream MDX consumers see identical Markdown for `.xls` and `.xlsx`
//! sources.
//!
//! Feature-gated behind `xls` (see `core/Cargo.toml`).

use std::io::{self, Cursor, Read};
use std::path::Path;

use calamine::{Data, Reader, Xls};

/// Parsed spreadsheet sheet.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

/// Metadata extracted from the workbook.
#[derive(Debug, Clone)]
pub struct XlsMetadata {
    pub sheet_count: usize,
}

/// Fully parsed workbook.
#[derive(Debug, Clone)]
pub struct XlsDocument {
    pub sheets: Vec<Sheet>,
    pub metadata: XlsMetadata,
}

/// XLS (BIFF8) parser backed by raw bytes.
pub struct XlsParser {
    data: Vec<u8>,
}

impl XlsParser {
    /// Open an XLS file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        Ok(Self { data })
    }

    /// Create a parser from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        Ok(Self { data })
    }

    /// Parse the workbook into an `XlsDocument`.
    pub fn parse(&self) -> io::Result<XlsDocument> {
        parse_xls(&self.data)
    }
}

/// Parse raw `.xls` (BIFF8) bytes into an `XlsDocument`.
pub fn parse_xls(data: &[u8]) -> io::Result<XlsDocument> {
    let cursor = Cursor::new(data);
    let mut workbook: Xls<_> = Xls::new(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let sheet_count = sheet_names.len();
    let mut sheets = Vec::with_capacity(sheet_count);

    for name in &sheet_names {
        let range = match workbook.worksheet_range(name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let mut rows: Vec<Vec<String>> = Vec::new();
        for row in range.rows() {
            let cells: Vec<String> = row
                .iter()
                .map(|cell| match cell {
                    Data::Empty => String::new(),
                    Data::String(s) => s.clone(),
                    Data::Float(f) => format_float(*f),
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => b.to_string(),
                    Data::Error(e) => format!("#ERR:{:?}", e),
                    Data::DateTime(dt) => excel_datetime_to_string(dt),
                    Data::DateTimeIso(s) => s.clone(),
                    Data::DurationIso(s) => s.clone(),
                })
                .collect();
            rows.push(cells);
        }

        // Trim trailing empty rows
        while rows.last().is_some_and(|r| r.iter().all(|c| c.is_empty())) {
            rows.pop();
        }

        // Trim trailing empty columns from every row
        if !rows.is_empty() {
            let max_col = trailing_content_col(&rows);
            for row in &mut rows {
                row.truncate(max_col + 1);
            }
        }

        sheets.push(Sheet { name: name.clone(), rows });
    }

    Ok(XlsDocument {
        sheets,
        metadata: XlsMetadata { sheet_count },
    })
}

impl XlsDocument {
    /// Render all sheets as Markdown with pipe tables.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        for (idx, sheet) in self.sheets.iter().enumerate() {
            if sheet.rows.is_empty() {
                continue;
            }

            if idx > 0 {
                out.push_str("\n\n");
            }

            out.push_str(&format!("## {}\n\n", sheet.name));
            out.push_str(&rows_to_pipe_table(&sheet.rows));
        }

        out
    }

    /// Convenience: render to MDX with front-matter.
    pub fn to_mdx(&self, source_name: &str) -> String {
        format!(
            "---\nformat: xls\nsource: \"{}\"\nsheets: {}\n---\n\n{}",
            source_name.replace('"', "\\\""),
            self.metadata.sheet_count,
            self.to_markdown(),
        )
    }
}

/// Detect whether raw bytes look like an XLS (BIFF8/OLE2) spreadsheet, as
/// opposed to another CFB-based format sharing the same OLE2 magic (HWP,
/// DOC, PPT, ...). Checks the CFB magic first, then peeks at root-level
/// stream names for `Workbook` (BIFF8/Excel 97+) or `Book` (BIFF5/Excel 95)
/// — the same "peek inside the container" idiom `detect_zip_format` uses
/// for ZIP-based formats in `main.rs`.
pub fn looks_like_xls(data: &[u8]) -> bool {
    const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if data.len() < 8 || data[..8] != CFB_MAGIC {
        return false;
    }
    let cursor = Cursor::new(data);
    let cf = match cfb::CompoundFile::open(cursor) {
        Ok(cf) => cf,
        Err(_) => return false,
    };
    cf.walk().any(|entry| {
        entry.is_stream()
            && matches!(entry.path().to_string_lossy().as_ref(), "/Workbook" | "/Book")
    })
}

// ---------------------------------------------------------------------------
// Helpers (mirrors crate::xlsx's private helpers — duplicated because that
// module's are private and it must not be modified; see task instructions).
// ---------------------------------------------------------------------------

/// Find the right-most column index that contains non-empty data.
fn trailing_content_col(rows: &[Vec<String>]) -> usize {
    let mut max = 0usize;
    for row in rows {
        for (i, cell) in row.iter().enumerate().rev() {
            if !cell.is_empty() {
                if i > max {
                    max = i;
                }
                break;
            }
        }
    }
    max
}

/// Format a float for human reading (strip `.0` suffix for integers).
fn format_float(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{:.0}", f)
    } else {
        f.to_string()
    }
}

/// Render a calamine `ExcelDateTime` cell as text.
///
/// Without calamine's `dates` cargo feature, `Data::DateTime`'s `Display`
/// prints the raw serial number (e.g. `45123`) instead of a date. We convert
/// the serial ourselves via chrono so date cells render as real dates.
/// Elapsed-time cells (durations) keep their numeric value.
fn excel_datetime_to_string(dt: &calamine::ExcelDateTime) -> String {
    if dt.is_duration() {
        return format_float(dt.as_f64());
    }
    excel_serial_to_iso(dt.as_f64())
}

/// Convert an Excel 1900-system serial date to `YYYY-MM-DD[ HH:MM:SS]`.
///
/// The `serial < 60.0` branch compensates for Excel's fictitious 1900-02-29
/// leap day (serials >= 60 are shifted one day forward from real dates).
fn excel_serial_to_iso(serial: f64) -> String {
    use chrono::{Duration, NaiveDate, NaiveTime};
    let days = if serial < 60.0 { serial } else { serial - 1.0 };
    let base = match NaiveDate::from_ymd_opt(1899, 12, 31) {
        Some(d) => d,
        None => return format_float(serial),
    };
    let date = match base.checked_add_signed(Duration::days(days.trunc() as i64)) {
        Some(d) => d,
        None => return format_float(serial),
    };
    let secs = (serial.fract() * 86_400.0).round() as i64;
    if secs <= 0 {
        date.format("%Y-%m-%d").to_string()
    } else {
        let t = NaiveTime::from_num_seconds_from_midnight_opt((secs % 86_400) as u32, 0)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        format!("{} {}", date.format("%Y-%m-%d"), t.format("%H:%M:%S"))
    }
}

/// Convert a 2-D grid of strings into a Markdown pipe table.
fn rows_to_pipe_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    // Determine column count from the widest row.
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }

    // Compute column widths (minimum 3 for the separator).
    let mut widths = vec![3usize; cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = cell.chars().count();
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }

    let mut out = String::new();

    // Header row (first row in the data).
    out.push('|');
    for (i, w) in widths.iter().enumerate() {
        let cell = rows[0].get(i).map(|s| s.as_str()).unwrap_or("");
        let escaped = escape_pipe(cell);
        out.push_str(&format!(" {:<width$} |", escaped, width = *w));
    }
    out.push('\n');

    // Separator row.
    out.push('|');
    for w in &widths {
        out.push_str(&format!(" {} |", "-".repeat(*w)));
    }
    out.push('\n');

    // Data rows.
    for row in &rows[1..] {
        out.push('|');
        for (i, w) in widths.iter().enumerate() {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let escaped = escape_pipe(cell);
            out.push_str(&format!(" {:<width$} |", escaped, width = *w));
        }
        out.push('\n');
    }

    out
}

/// Escape pipe characters inside table cells.
fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_bytes() -> Vec<u8> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/xls/test_korean.xls"
        );
        std::fs::read(path).expect("test fixture tests/fixtures/xls/test_korean.xls missing")
    }

    #[test]
    fn test_looks_like_xls_true_for_fixture() {
        let data = fixture_bytes();
        assert!(looks_like_xls(&data));
    }

    #[test]
    fn test_looks_like_xls_false_for_non_cfb() {
        assert!(!looks_like_xls(b"not a compound file"));
        assert!(!looks_like_xls(b"PK\x03\x04"));
    }

    #[test]
    fn test_parse_xls_sheet_count_and_names() {
        let data = fixture_bytes();
        let doc = parse_xls(&data).expect("parse_xls should succeed on fixture");
        assert_eq!(doc.metadata.sheet_count, 3);
        let names: Vec<&str> = doc.sheets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["데이터시트", "Sheet2", "병합셀"]);
    }

    #[test]
    fn test_parse_xls_korean_text_and_numbers() {
        let data = fixture_bytes();
        let doc = parse_xls(&data).expect("parse_xls should succeed on fixture");
        let sheet1 = &doc.sheets[0];

        // Header row
        assert_eq!(sheet1.rows[0], vec!["이름", "나이", "가입일", "점수"]);
        // First data row: Korean name preserved, int as plain digits, float as-is
        assert_eq!(sheet1.rows[1][0], "홍길동");
        assert_eq!(sheet1.rows[1][1], "30");
        assert_eq!(sheet1.rows[1][3], "95.5");

        let sheet2 = &doc.sheets[1];
        assert_eq!(sheet2.rows[1][0], "노트북");
        assert_eq!(sheet2.rows[1][1], "1500000");
    }

    #[test]
    fn test_parse_xls_merged_and_empty_cells() {
        let data = fixture_bytes();
        let doc = parse_xls(&data).expect("parse_xls should succeed on fixture");
        let merged = &doc.sheets[2];

        // Merged header: calamine repeats the merge anchor's value only in
        // the top-left cell; other cells in the merge range read as empty —
        // identical behavior to xlsx::XlsxDocument (calamine does not
        // synthesize merge-fill values for either format).
        assert_eq!(merged.rows[0][0], "병합된 헤더");
        // Row "A", <empty>, "C" — the empty cell in the middle must survive
        // as an empty string, not be silently dropped.
        assert_eq!(merged.rows[1], vec!["A", "", "C"]);
    }

    #[test]
    fn test_parse_xls_to_mdx_frontmatter() {
        let data = fixture_bytes();
        let doc = parse_xls(&data).expect("parse_xls should succeed on fixture");
        let mdx = doc.to_mdx("test_korean.xls");
        assert!(mdx.starts_with("---\nformat: xls\n"));
        assert!(mdx.contains("source: \"test_korean.xls\""));
        assert!(mdx.contains("sheets: 3"));
        assert!(mdx.contains("## 데이터시트"));
    }

    #[test]
    fn test_parse_xls_rejects_garbage() {
        let result = parse_xls(b"not an xls file at all");
        assert!(result.is_err());
    }
}
