//! CSV / TSV parser.
//!
//! Auto-detects delimiter (comma vs tab) and renders the data as a Markdown
//! pipe table.

use std::io::{self, Read};
use std::path::Path;

/// Parsed CSV document.
#[derive(Debug, Clone)]
pub struct CsvDocument {
    pub rows: Vec<Vec<String>>,
    pub has_header: bool,
}

/// CSV / TSV parser.
pub struct CsvParser {
    data: Vec<u8>,
    delimiter: u8,
}

impl CsvParser {
    /// Open a CSV file from disk with auto-detected delimiter.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        let delimiter = detect_delimiter(&data);
        Ok(Self { data, delimiter })
    }

    /// Create a parser from raw bytes (defaults to comma delimiter).
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let delimiter = detect_delimiter(&data);
        Ok(Self { data, delimiter })
    }

    /// Parse the CSV data into a `CsvDocument`.
    pub fn parse(&self) -> io::Result<CsvDocument> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(self.data.as_slice());

        let mut rows: Vec<Vec<String>> = Vec::new();
        for result in reader.records() {
            let record = result
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            let cells: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            rows.push(cells);
        }

        // Trim trailing empty rows.
        while rows.last().map_or(false, |r| r.iter().all(|c| c.trim().is_empty())) {
            rows.pop();
        }

        Ok(CsvDocument {
            rows,
            has_header: true, // Treat first row as header by default.
        })
    }
}

impl CsvDocument {
    /// Render the CSV data as a Markdown pipe table.
    pub fn to_markdown(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let cols = self.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if cols == 0 {
            return String::new();
        }

        // Compute column widths.
        let mut widths = vec![3usize; cols];
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                let w = cell.chars().count();
                if w > widths[i] {
                    widths[i] = w;
                }
            }
        }

        let mut out = String::new();

        // Header.
        out.push('|');
        for (i, w) in widths.iter().enumerate() {
            let cell = self.rows[0].get(i).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(" {:<width$} |", escape_pipe(cell), width = *w));
        }
        out.push('\n');

        // Separator.
        out.push('|');
        for w in &widths {
            out.push_str(&format!(" {} |", "-".repeat(*w)));
        }
        out.push('\n');

        // Data.
        for row in &self.rows[1..] {
            out.push('|');
            for (i, w) in widths.iter().enumerate() {
                let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                out.push_str(&format!(" {:<width$} |", escape_pipe(cell), width = *w));
            }
            out.push('\n');
        }

        out
    }

    /// Convenience: render to MDX with front-matter.
    pub fn to_mdx(&self, source_name: &str) -> String {
        format!(
            "---\nformat: csv\nsource: \"{}\"\nrows: {}\ncolumns: {}\n---\n\n{}",
            source_name.replace('"', "\\\""),
            self.rows.len(),
            self.rows.first().map(|r| r.len()).unwrap_or(0),
            self.to_markdown(),
        )
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Auto-detect delimiter by counting commas vs tabs on the first line.
fn detect_delimiter(data: &[u8]) -> u8 {
    // Decode first line.
    let first_line_end = data.iter().position(|&b| b == b'\n').unwrap_or(data.len());
    let first_line = &data[..first_line_end];

    let commas = first_line.iter().filter(|&&b| b == b',').count();
    let tabs = first_line.iter().filter(|&&b| b == b'\t').count();

    if tabs > commas { b'\t' } else { b',' }
}

fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_delimiter_comma() {
        assert_eq!(detect_delimiter(b"a,b,c\n1,2,3"), b',');
    }

    #[test]
    fn test_detect_delimiter_tab() {
        assert_eq!(detect_delimiter(b"a\tb\tc\n1\t2\t3"), b'\t');
    }

    #[test]
    fn test_csv_parse_and_markdown() {
        let data = b"Name,Age\nAlice,30\nBob,25";
        let parser = CsvParser::from_bytes(data.to_vec()).unwrap();
        let doc = parser.parse().unwrap();
        let md = doc.to_markdown();
        assert!(md.contains("| Name"));
        assert!(md.contains("| Alice"));
        assert!(md.contains("| ---"));
        assert_eq!(doc.rows.len(), 3);
    }

    #[test]
    fn test_tsv_parse() {
        let data = b"Name\tAge\nAlice\t30";
        let parser = CsvParser::from_bytes(data.to_vec()).unwrap();
        let doc = parser.parse().unwrap();
        assert_eq!(doc.rows.len(), 2);
        assert_eq!(doc.rows[0], vec!["Name", "Age"]);
    }

    #[test]
    fn test_escape_pipe() {
        assert_eq!(escape_pipe("a|b"), "a\\|b");
    }
}
