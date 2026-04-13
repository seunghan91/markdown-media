//! XLSX (Excel) spreadsheet parser.
//!
//! Converts `.xlsx` / `.xls` files to Markdown pipe tables using `calamine`.

use std::io::{self, Cursor, Read};
use std::path::Path;

use calamine::{open_workbook_auto_from_rs, Data, Reader};

/// Parsed spreadsheet sheet.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

/// Metadata extracted from the workbook.
#[derive(Debug, Clone)]
pub struct XlsxMetadata {
    pub sheet_count: usize,
}

/// Fully parsed workbook.
#[derive(Debug, Clone)]
pub struct XlsxDocument {
    pub sheets: Vec<Sheet>,
    pub metadata: XlsxMetadata,
}

/// XLSX parser backed by raw bytes.
pub struct XlsxParser {
    data: Vec<u8>,
}

impl XlsxParser {
    /// Open an XLSX file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        Ok(Self { data })
    }

    /// Create a parser from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        Ok(Self { data })
    }

    /// Parse the workbook into an `XlsxDocument`.
    pub fn parse(&self) -> io::Result<XlsxDocument> {
        let cursor = Cursor::new(&self.data);
        let mut workbook = open_workbook_auto_from_rs(cursor)
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
                        Data::DateTime(dt) => format!("{}", dt),
                        Data::DateTimeIso(s) => s.clone(),
                        Data::DurationIso(s) => s.clone(),
                    })
                    .collect();
                rows.push(cells);
            }

            // Trim trailing empty rows
            while rows.last().map_or(false, |r| r.iter().all(|c| c.is_empty())) {
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

        Ok(XlsxDocument {
            sheets,
            metadata: XlsxMetadata { sheet_count },
        })
    }
}

impl XlsxDocument {
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
            "---\nformat: xlsx\nsource: \"{}\"\nsheets: {}\n---\n\n{}",
            source_name.replace('"', "\\\""),
            self.metadata.sheet_count,
            self.to_markdown(),
        )
    }
}

// ---------------------------------------------------------------------------
// Helpers
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

    #[test]
    fn test_rows_to_pipe_table() {
        let rows = vec![
            vec!["Name".into(), "Age".into()],
            vec!["Alice".into(), "30".into()],
            vec!["Bob".into(), "25".into()],
        ];
        let table = rows_to_pipe_table(&rows);
        assert!(table.contains("| Name"));
        assert!(table.contains("| ---"));
        assert!(table.contains("| Alice"));
    }

    #[test]
    fn test_format_float_integer() {
        assert_eq!(format_float(42.0), "42");
        assert_eq!(format_float(3.14), "3.14");
    }

    #[test]
    fn test_trailing_content_col() {
        let rows = vec![
            vec!["a".into(), "b".into(), "".into(), "".into()],
            vec!["c".into(), "".into(), "".into(), "".into()],
        ];
        assert_eq!(trailing_content_col(&rows), 1);
    }

    #[test]
    fn test_escape_pipe() {
        assert_eq!(escape_pipe("a|b"), "a\\|b");
    }
}
