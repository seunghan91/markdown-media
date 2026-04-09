//! Bounded I/O and decompression utilities.
//!
//! Defends against decompression bombs (HWP raw deflate, HWPX ZIP entries,
//! PDF FlateDecode) by enforcing hard output-size ceilings. A malicious
//! document could otherwise expand a few KB of compressed input into
//! multi-GB output and OOM the host.
//!
//! All functions return `io::Error` with `ErrorKind::InvalidData` when the
//! limit is exceeded, so callers can surface a clean "file too large /
//! suspicious compression ratio" error instead of crashing.

use flate2::read::{DeflateDecoder, ZlibDecoder};
use miniz_oxide::inflate::DecompressError;
use std::io::{self, Read};

/// Default per-stream ceilings. Tuned for legal documents — real-world
/// Korean law/contract files live well under these.
pub const MAX_HWP_SECTION: usize = 256 * 1024 * 1024; // 256 MB
pub const MAX_HWPX_XML: usize = 32 * 1024 * 1024;      //  32 MB
pub const MAX_HWPX_BINDATA: usize = 64 * 1024 * 1024;  //  64 MB
pub const MAX_PDF_STREAM: usize = 128 * 1024 * 1024;   // 128 MB
pub const MAX_PDF_FILE: usize = 512 * 1024 * 1024;     // 512 MB

/// Read from `reader` into a `Vec<u8>`, aborting if more than `max` bytes
/// are produced. Uses `Read::take` so we never allocate beyond `max + 1`.
pub fn read_limited<R: Read>(reader: &mut R, max: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    // Take(max+1) lets us detect overflow: if we managed to read max+1 bytes,
    // the source had more than the limit.
    reader
        .take((max as u64).saturating_add(1))
        .read_to_end(&mut buf)?;
    if buf.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("decompressed stream exceeds limit of {} bytes", max),
        ));
    }
    Ok(buf)
}

/// Read a `String` from `reader` with a byte-length ceiling. Rejects
/// invalid UTF-8 with `InvalidData`.
pub fn read_limited_to_string<R: Read>(reader: &mut R, max: usize) -> io::Result<String> {
    let bytes = read_limited(reader, max)?;
    String::from_utf8(bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

/// Zlib (with header) decompression capped at `max` bytes of output.
pub fn decompress_zlib_limited(data: &[u8], max: usize) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    read_limited(&mut decoder, max)
}

/// Flate2 raw-deflate (`wbits=-15`) decompression capped at `max`.
pub fn decompress_deflate_limited(data: &[u8], max: usize) -> io::Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(data);
    read_limited(&mut decoder, max)
}

/// Raw-deflate via `miniz_oxide` (matches Python `zlib.decompress(data, -15)`)
/// with a hard output ceiling. This replaces the unbounded growth loop in
/// the old `decompress_raw_deflate`.
pub fn decompress_raw_deflate_limited(
    data: &[u8],
    max: usize,
) -> Result<Vec<u8>, DecompressError> {
    use miniz_oxide::inflate::core::{decompress, inflate_flags, DecompressorOxide};
    use miniz_oxide::inflate::TINFLStatus;

    // Start with a modest guess (10x, capped at max). We'll grow on demand
    // but never beyond `max`.
    let initial = data
        .len()
        .saturating_mul(10)
        .max(4096)
        .min(max);
    let mut output = vec![0u8; initial];
    let mut decompressor = DecompressorOxide::new();

    let mut in_pos = 0usize;
    let mut out_pos = 0usize;

    loop {
        let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
            | if in_pos < data.len() {
                inflate_flags::TINFL_FLAG_HAS_MORE_INPUT
            } else {
                0
            };

        let (status, bytes_read, bytes_written) = decompress(
            &mut decompressor,
            &data[in_pos..],
            &mut output[out_pos..],
            out_pos,
            flags,
        );

        in_pos += bytes_read;
        out_pos += bytes_written;

        match status {
            TINFLStatus::Done => {
                output.truncate(out_pos);
                return Ok(output);
            }
            TINFLStatus::HasMoreOutput => {
                if output.len() >= max {
                    // Would exceed ceiling — bail.
                    return Err(DecompressError {
                        status: TINFLStatus::Failed,
                        output,
                    });
                }
                let new_size = output.len().saturating_mul(2).min(max);
                output.resize(new_size, 0);
            }
            TINFLStatus::NeedsMoreInput => {
                // Truncated but usable output — return what we have.
                output.truncate(out_pos);
                return Ok(output);
            }
            _ => {
                return Err(DecompressError {
                    status,
                    output: Vec::new(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_limited_under_cap() {
        let data = vec![1u8, 2, 3, 4, 5];
        let mut cursor = std::io::Cursor::new(data.clone());
        let out = read_limited(&mut cursor, 10).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn read_limited_over_cap_errors() {
        let data = vec![0u8; 100];
        let mut cursor = std::io::Cursor::new(data);
        let err = read_limited(&mut cursor, 50).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn read_limited_exactly_at_cap() {
        let data = vec![42u8; 50];
        let mut cursor = std::io::Cursor::new(data.clone());
        let out = read_limited(&mut cursor, 50).unwrap();
        assert_eq!(out.len(), 50);
        assert_eq!(out, data);
    }
}
