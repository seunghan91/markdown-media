use crate::hwp::cfb_lenient::LenientCfb;
use crate::utils::bounded_io::{
    decompress_raw_deflate_limited, read_limited, MAX_HWP_SECTION,
};
use cfb::CompoundFile;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// HWP FileHeader flags (offset 36-39)
#[derive(Debug, Clone, Copy)]
pub struct HwpFlags {
    pub compressed: bool,
    pub encrypted: bool,
    pub distributed: bool,
    pub script_saved: bool,
    pub drm_protected: bool,
    pub xml_template: bool,
    pub history: bool,
    pub signature: bool,
    pub certificate_encrypted: bool,
    pub certificate_drm: bool,
    pub ccl: bool,
    pub mobile_optimized: bool,
    pub private_info_security: bool,
    pub track_changes: bool,
    pub kogl: bool,
    pub video_control: bool,
    pub order_field_control: bool,
}

impl Default for HwpFlags {
    fn default() -> Self {
        HwpFlags {
            compressed: true, // Default to true as most HWP files are compressed
            encrypted: false,
            distributed: false,
            script_saved: false,
            drm_protected: false,
            xml_template: false,
            history: false,
            signature: false,
            certificate_encrypted: false,
            certificate_drm: false,
            ccl: false,
            mobile_optimized: false,
            private_info_security: false,
            track_changes: false,
            kogl: false,
            video_control: false,
            order_field_control: false,
        }
    }
}

impl HwpFlags {
    pub fn from_bytes(data: &[u8]) -> Self {
        if data.len() < 4 {
            return Self::default();
        }
        
        let flags = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        
        HwpFlags {
            compressed: (flags & 0x01) != 0,
            encrypted: (flags & 0x02) != 0,
            distributed: (flags & 0x04) != 0,
            script_saved: (flags & 0x08) != 0,
            drm_protected: (flags & 0x10) != 0,
            xml_template: (flags & 0x20) != 0,
            history: (flags & 0x40) != 0,
            signature: (flags & 0x80) != 0,
            certificate_encrypted: (flags & 0x100) != 0,
            certificate_drm: (flags & 0x200) != 0,
            ccl: (flags & 0x400) != 0,
            mobile_optimized: (flags & 0x800) != 0,
            private_info_security: (flags & 0x1000) != 0,
            track_changes: (flags & 0x2000) != 0,
            kogl: (flags & 0x4000) != 0,
            video_control: (flags & 0x8000) != 0,
            order_field_control: (flags & 0x10000) != 0,
        }
    }
}

/// Storage backend for an opened HWP file.
///
/// We try the strict `cfb` crate first because it's battle-tested and
/// correctly handles the common case. When strict validation rejects a
/// file (damaged FAT, non-conformant dir entries, etc.), we fall back to
/// [`LenientCfb`] which walks the structure directly with guards instead
/// of hard errors. This fallback recovers the large fraction of real-world
/// Korean government documents that strict parsers reject.
enum OleBackend {
    Standard(CompoundFile<File>),
    /// In-memory OLE compound file (for WASM / from_bytes).
    Memory(CompoundFile<std::io::Cursor<Vec<u8>>>),
    Lenient(Box<LenientCfb>),
}

/// OLE 파일 구조를 분석하고 스트림을 추출합니다
pub struct OleReader {
    backend: OleBackend,
    flags: HwpFlags,
}

impl OleReader {
    /// HWP 파일을 OLE 구조로 엽니다.
    ///
    /// Standard CFB 파싱을 먼저 시도하고, 실패 시 lenient 폴백으로
    /// 손상된 FAT/디렉토리를 가진 파일도 복구 시도한다.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_ref = path.as_ref();

        // Strict path first — the happy case.
        let file = File::open(path_ref)?;
        let strict_result = CompoundFile::open(file);

        let backend = match strict_result {
            Ok(cf) => OleBackend::Standard(cf),
            Err(_strict_err) => {
                // Reopen & slurp; LenientCfb owns the bytes.
                let mut file = File::open(path_ref)?;
                let data = read_limited(&mut file, MAX_HWP_SECTION)?;
                let lenient = LenientCfb::parse(data).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("both strict and lenient CFB parse failed: {}", e),
                    )
                })?;
                OleBackend::Lenient(Box::new(lenient))
            }
        };

        let mut reader = OleReader {
            backend,
            flags: HwpFlags::default(),
        };

        // Try to read flags from FileHeader (same stream name in both backends).
        if let Ok(header) = reader.read_stream("FileHeader") {
            if header.len() >= 40 {
                reader.flags = HwpFlags::from_bytes(&header[36..40]);
            }
        }

        Ok(reader)
    }

    /// Create an OleReader from in-memory data.
    ///
    /// Tries strict CFB parsing first via `CompoundFile::open(Cursor)`,
    /// then falls back to `LenientCfb` if the strict parser rejects the
    /// data. Used for WASM and other sandboxed environments.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let cursor = std::io::Cursor::new(data.clone());
        let strict_result = CompoundFile::open(cursor);

        let backend = match strict_result {
            Ok(cf) => OleBackend::Memory(cf),
            Err(_strict_err) => {
                let lenient = LenientCfb::parse(data).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("both strict and lenient CFB parse failed: {}", e),
                    )
                })?;
                OleBackend::Lenient(Box::new(lenient))
            }
        };

        let mut reader = OleReader {
            backend,
            flags: HwpFlags::default(),
        };

        if let Ok(header) = reader.read_stream("FileHeader") {
            if header.len() >= 40 {
                reader.flags = HwpFlags::from_bytes(&header[36..40]);
            }
        }

        Ok(reader)
    }

    /// Get file flags
    pub fn flags(&self) -> &HwpFlags {
        &self.flags
    }

    /// True when we fell back to the lenient parser — useful for logging
    /// / metadata reporting.
    pub fn is_lenient(&self) -> bool {
        matches!(self.backend, OleBackend::Lenient(_))
    }

    /// 모든 스트림 이름 목록을 가져옵니다
    pub fn list_streams(&self) -> Vec<String> {
        match &self.backend {
            OleBackend::Standard(cf) => cf
                .walk()
                .filter_map(|entry| {
                    if entry.is_stream() {
                        Some(entry.path().to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect(),
            OleBackend::Memory(cf) => cf
                .walk()
                .filter_map(|entry| {
                    if entry.is_stream() {
                        Some(entry.path().to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect(),
            OleBackend::Lenient(lcfb) => lcfb.stream_names(),
        }
    }

    /// 특정 스트림의 내용을 읽습니다 (raw, uncompressed).
    /// Capped at `MAX_HWP_SECTION` so a malformed CFB with a gigantic stream
    /// cannot exhaust memory.
    pub fn read_stream(&mut self, stream_name: &str) -> io::Result<Vec<u8>> {
        match &mut self.backend {
            OleBackend::Standard(cf) => {
                let mut stream = cf
                    .open_stream(stream_name)
                    .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
                read_limited(&mut stream, MAX_HWP_SECTION)
            }
            OleBackend::Memory(cf) => {
                let mut stream = cf
                    .open_stream(stream_name)
                    .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
                read_limited(&mut stream, MAX_HWP_SECTION)
            }
            OleBackend::Lenient(lcfb) => lcfb.find_stream(stream_name).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("stream not found (lenient): {}", stream_name),
                )
            }),
        }
    }

    /// 압축된 스트림을 읽고 해제합니다
    pub fn read_compressed_stream(&mut self, stream_name: &str) -> io::Result<Vec<u8>> {
        let raw_data = self.read_stream(stream_name)?;
        
        if !self.flags.compressed {
            return Ok(raw_data);
        }
        
        // Decompress with zlib
        decompress_zlib(&raw_data)
    }

    /// FileHeader 스트림을 읽습니다 (HWP 메타데이터)
    pub fn read_file_header(&mut self) -> io::Result<Vec<u8>> {
        // FileHeader is never compressed
        self.read_stream("FileHeader")
    }

    /// DocInfo 스트림을 읽습니다 (문서 정보)
    pub fn read_doc_info(&mut self) -> io::Result<Vec<u8>> {
        self.read_compressed_stream("DocInfo")
    }

    /// BodyText 섹션을 읽습니다
    pub fn read_body_text(&mut self, section: usize) -> io::Result<Vec<u8>> {
        let stream_name = format!("BodyText/Section{}", section);
        self.read_compressed_stream(&stream_name)
    }

    /// ViewText 섹션(배포용 문서의 암호화된 본문)을 raw 바이트로 읽습니다.
    /// 복호화는 `crate::hwp::crypto::decrypt_view_text` 에서 수행하며,
    /// 이 스트림 자체는 AES 복호화 이전의 원본이므로 압축 해제를 하지
    /// 않고 그대로 돌려줍니다.
    pub fn read_view_text_raw(&mut self, section: usize) -> io::Result<Vec<u8>> {
        let stream_name = format!("ViewText/Section{}", section);
        self.read_stream(&stream_name)
    }

    /// ViewText 섹션 개수 (배포용 문서 전용). 일반 BodyText 개수와는
    /// 별도 카운트된다.
    pub fn view_section_count(&self) -> usize {
        match &self.backend {
            OleBackend::Standard(cf) => cf
                .walk()
                .filter(|entry| {
                    entry.is_stream()
                        && entry
                            .path()
                            .to_string_lossy()
                            .starts_with("/ViewText/Section")
                })
                .count(),
            OleBackend::Memory(cf) => cf
                .walk()
                .filter(|entry| {
                    entry.is_stream()
                        && entry
                            .path()
                            .to_string_lossy()
                            .starts_with("/ViewText/Section")
                })
                .count(),
            OleBackend::Lenient(lcfb) => {
                // Lenient parser stores stream names without the parent
                // storage prefix. In distribution-locked files only one of
                // {BodyText, ViewText} is present, so a flat "Section*"
                // count is accurate when callers dispatch on
                // `flags.distributed` as HwpParser::extract_text does.
                lcfb.stream_names()
                    .iter()
                    .filter(|n| n.starts_with("Section"))
                    .count()
            }
        }
    }

    /// BinData 스트림을 읽습니다 (이미지, OLE 객체 등)
    pub fn read_bin_data(&mut self, name: &str) -> io::Result<Vec<u8>> {
        let stream_name = format!("BinData/{}", name);
        let data = self.read_stream(&stream_name)?;
        
        // BinData may or may not be compressed
        // Try to decompress, fall back to raw data
        decompress_zlib(&data).or(Ok(data))
    }

    /// 모든 BinData 스트림 이름을 가져옵니다
    pub fn list_bin_data(&self) -> Vec<String> {
        match &self.backend {
            OleBackend::Standard(cf) => cf
                .walk()
                .filter_map(|entry| {
                    let path = entry.path().to_string_lossy().to_string();
                    if entry.is_stream() && path.starts_with("/BinData/") {
                        Some(path.strip_prefix("/BinData/").unwrap_or(&path).to_string())
                    } else {
                        None
                    }
                })
                .collect(),
            OleBackend::Memory(cf) => cf
                .walk()
                .filter_map(|entry| {
                    let path = entry.path().to_string_lossy().to_string();
                    if entry.is_stream() && path.starts_with("/BinData/") {
                        Some(path.strip_prefix("/BinData/").unwrap_or(&path).to_string())
                    } else {
                        None
                    }
                })
                .collect(),
            OleBackend::Lenient(lcfb) => {
                // Lenient parser loses the parent-storage prefix. HWP BinData
                // streams are conventionally named BIN0001.{ext} etc., so we
                // use the name pattern as a heuristic filter.
                lcfb.stream_names()
                    .into_iter()
                    .filter(|n| n.starts_with("BIN"))
                    .collect()
            }
        }
    }

    /// Read the OLE2 SummaryInformation stream (`\u{0005}HwpSummaryInformation`).
    /// Returns raw bytes; parsing into propIds is the caller's job.
    pub fn read_summary_information(&mut self) -> io::Result<Vec<u8>> {
        // The leading byte is the binary 0x05 control char per OLE2 spec for
        // standard property streams.
        self.read_stream("\u{0005}HwpSummaryInformation")
    }

    /// Summary section count (BodyText sections)
    pub fn section_count(&self) -> usize {
        match &self.backend {
            OleBackend::Standard(cf) => cf
                .walk()
                .filter(|entry| {
                    entry.is_stream()
                        && entry
                            .path()
                            .to_string_lossy()
                            .starts_with("/BodyText/Section")
                })
                .count(),
            OleBackend::Memory(cf) => cf
                .walk()
                .filter(|entry| {
                    entry.is_stream()
                        && entry
                            .path()
                            .to_string_lossy()
                            .starts_with("/BodyText/Section")
                })
                .count(),
            OleBackend::Lenient(lcfb) => {
                // Same caveat as view_section_count: HwpParser dispatches by
                // the distribution flag so the flat Section count is used
                // for whichever stream family is actually present.
                lcfb.stream_names()
                    .iter()
                    .filter(|n| n.starts_with("Section"))
                    .count()
            }
        }
    }
}

/// Decompress zlib-compressed data with a hard output ceiling.
///
/// HWP files use raw deflate (equivalent to Python's `zlib.decompress(data, -15)`).
/// All three strategies (zlib-header, raw deflate via miniz_oxide, flate2
/// DeflateDecoder fallback) are capped at `MAX_HWP_SECTION` to defeat
/// decompression bombs.
pub fn decompress_zlib(data: &[u8]) -> io::Result<Vec<u8>> {
    // 1) Try zlib with header (standard zlib format)
    let mut decoder = ZlibDecoder::new(data);
    let zlib_attempt: io::Result<Vec<u8>> = read_limited(&mut decoder, MAX_HWP_SECTION);
    if let Ok(out) = zlib_attempt {
        if !out.is_empty() {
            return Ok(out);
        }
    }

    // 2) Try raw deflate using miniz_oxide (equivalent to Python's wbits=-15).
    //    This is what HWP files actually use most of the time.
    let raw_attempt: Result<Vec<u8>, _> =
        decompress_raw_deflate_limited(data, MAX_HWP_SECTION);
    if let Ok(result) = raw_attempt {
        if !result.is_empty() {
            return Ok(result);
        }
    }

    // 3) Fallback: flate2's DeflateDecoder via read_limited.
    let mut decoder = DeflateDecoder::new(data);
    read_limited(&mut decoder, MAX_HWP_SECTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ole_structure() {
        // 실제 HWP 파일이 있어야 테스트 가능
        // TODO: 샘플 HWP 파일 추가
    }

    #[test]
    fn test_hwp_flags_parsing() {
        // Test flags: compressed=true, encrypted=false
        let data = vec![0x01, 0x00, 0x00, 0x00];
        let flags = HwpFlags::from_bytes(&data);
        assert!(flags.compressed);
        assert!(!flags.encrypted);
        
        // Test flags: compressed=true, encrypted=true
        let data = vec![0x03, 0x00, 0x00, 0x00];
        let flags = HwpFlags::from_bytes(&data);
        assert!(flags.compressed);
        assert!(flags.encrypted);
    }
}
