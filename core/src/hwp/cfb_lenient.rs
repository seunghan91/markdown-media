//! Lenient CFB (Compound File Binary / OLE2) fallback parser.
//!
//! The standard `cfb` crate rejects HWP files whose FAT / directory tables
//! fail strict validation — which unfortunately includes a large fraction of
//! real-world Korean government documents that were saved by older or
//! non-conformant editors. This module is a forgiving reimplementation that
//! walks the FAT directly, skips obvious corruption, and applies cycle /
//! size guards so a malicious input cannot OOM us.
//!
//! Algorithm ported from `reference/kordoc/src/hwp5/cfb-lenient.ts`, which
//! is itself a port of rhwp (MIT) `src/parser/cfb_reader.rs::LenientCfbReader`.
//! Spec reference: MS-CFB
//! (https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb).

use std::io;

// ── Constants ────────────────────────────────────────────────────────────────

const CFB_MAGIC: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
const END_OF_CHAIN: u32 = 0xffff_fffe;
const FREE_SECT: u32 = 0xffff_ffff;

/// Cycle-detection cap on chain walking.
const MAX_CHAIN_LENGTH: usize = 1_000_000;
/// Directory entry cap (each entry is 128B → 12.8 MB max dir stream).
const MAX_DIR_ENTRIES: usize = 100_000;
/// Per-stream ceiling — defensive, applied on top of the bounded_io caps.
const MAX_STREAM_SIZE: usize = 100 * 1024 * 1024;

// Directory entry type codes per MS-CFB.
const TYPE_STORAGE: u8 = 1;
const TYPE_STREAM: u8 = 2;
const TYPE_ROOT: u8 = 5;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub entry_type: u8,
    pub start_sector: u32,
    pub size: u64,
}

/// A parsed CFB container. Owns the raw file bytes so that stream reads are
/// just buffer slicing — no further I/O.
#[derive(Debug)]
pub struct LenientCfb {
    data: Vec<u8>,
    sector_size: usize,
    mini_sector_size: usize,
    mini_stream_cutoff: u32,
    fat_table: Vec<u32>,
    dir_entries: Vec<DirEntry>,
    // Lazily computed
    mini_fat_table: Option<Vec<u32>>,
    mini_stream: Option<Vec<u8>>,
    first_mini_fat_sector: u32,
    mini_fat_sector_count: u32,
}

impl LenientCfb {
    /// Parse the raw bytes of a CFB file. Returns a container that can then
    /// be queried by stream name via [`find_stream`](Self::find_stream).
    pub fn parse(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 512 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CFB file too short (< 512 bytes)",
            ));
        }
        if data[..8] != CFB_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CFB magic bytes mismatch",
            ));
        }

        // Header fields (all little-endian).
        let sector_size_shift = read_u16_le(&data, 30);
        if !(7..=16).contains(&sector_size_shift) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid sector size shift: {}", sector_size_shift),
            ));
        }
        let sector_size = 1usize << sector_size_shift;
        let mini_sector_size_shift = read_u16_le(&data, 32);
        if mini_sector_size_shift > 16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid mini sector size shift: {}", mini_sector_size_shift),
            ));
        }
        let mini_sector_size = 1usize << mini_sector_size_shift;

        let fat_sector_count = read_u32_le(&data, 44) as usize;
        let first_dir_sector = read_u32_le(&data, 48);
        let mini_stream_cutoff = read_u32_le(&data, 56); // usually 4096
        let first_mini_fat_sector = read_u32_le(&data, 60);
        let mini_fat_sector_count = read_u32_le(&data, 64);
        let first_difat_sector = read_u32_le(&data, 68);
        let difat_sector_count = read_u32_le(&data, 72) as usize;

        // ── Build list of FAT sectors from header DIFAT + DIFAT chain ──
        let fat_sectors = build_fat_sector_list(
            &data,
            sector_size,
            fat_sector_count,
            first_difat_sector,
            difat_sector_count,
        );

        // ── Build flat FAT table ──
        let entries_per_fat_sector = sector_size / 4;
        let fat_table = build_fat_table(&data, sector_size, &fat_sectors, entries_per_fat_sector);

        // ── Walk directory chain ──
        let dir_data = read_chain_static(
            &data,
            sector_size,
            &fat_table,
            first_dir_sector,
            (MAX_DIR_ENTRIES * 128) as u64,
        )?;

        let dir_entries = parse_dir_entries(&dir_data);

        Ok(Self {
            data,
            sector_size,
            mini_sector_size,
            mini_stream_cutoff,
            fat_table,
            dir_entries,
            mini_fat_table: None,
            mini_stream: None,
            first_mini_fat_sector,
            mini_fat_sector_count,
        })
    }

    /// Every stream-type directory entry (skips storage/root entries).
    pub fn entries(&self) -> Vec<&DirEntry> {
        self.dir_entries
            .iter()
            .filter(|e| e.entry_type == TYPE_STREAM)
            .collect()
    }

    /// List stream names. Mirrors `OleReader::list_streams`.
    pub fn stream_names(&self) -> Vec<String> {
        self.entries()
            .into_iter()
            .map(|e| e.name.clone())
            .collect()
    }

    /// Lookup a stream by its CFB path. HWP paths have at most 2 components
    /// (`BodyText/Section0`, `BinData/BIN0001`, …). We do a flat
    /// name-based match — permissive: when multiple entries share the
    /// stream name (unusual), the first one wins.
    pub fn find_stream(&mut self, path: &str) -> Option<Vec<u8>> {
        let normalized = path.trim_start_matches('/');
        let parts: Vec<&str> = normalized.split('/').collect();

        let entry_idx = if parts.len() == 1 {
            self.dir_entries.iter().position(|e| {
                e.entry_type == TYPE_STREAM && e.name == parts[0]
            })
        } else {
            // Try the joined tail first (e.g. "Section0" in
            // "BodyText/Section0"), then fall back to last component.
            let tail = parts[1..].join("/");
            let last = parts.last().copied().unwrap_or("");
            self.dir_entries
                .iter()
                .position(|e| e.entry_type == TYPE_STREAM && e.name == tail)
                .or_else(|| {
                    self.dir_entries.iter().position(|e| {
                        e.entry_type == TYPE_STREAM && e.name == last
                    })
                })
        }?;

        let entry = self.dir_entries[entry_idx].clone();
        self.read_stream_data(&entry).ok()
    }

    /// Stream count by prefix (used by OleReader::section_count parity).
    pub fn stream_count_with_prefix(&self, prefix: &str) -> usize {
        self.dir_entries
            .iter()
            .filter(|e| e.entry_type == TYPE_STREAM && e.name.starts_with(prefix))
            .count()
    }

    // ── Private helpers ──

    fn read_stream_data(&mut self, entry: &DirEntry) -> io::Result<Vec<u8>> {
        if entry.size == 0 {
            return Ok(Vec::new());
        }
        if entry.size > MAX_STREAM_SIZE as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("stream exceeds {} bytes", MAX_STREAM_SIZE),
            ));
        }

        // Mini stream path: entries smaller than the cutoff live in the
        // mini stream, which is itself a chain of regular sectors rooted at
        // the Root directory entry.
        if (entry.size as u32) < self.mini_stream_cutoff {
            if let Ok(mini) = self.read_mini_stream(entry.start_sector, entry.size) {
                if !mini.is_empty() {
                    return Ok(mini);
                }
            }
            // lenient fallback: try the regular chain
        }

        read_chain_static(
            &self.data,
            self.sector_size,
            &self.fat_table,
            entry.start_sector,
            entry.size,
        )
    }

    fn ensure_mini_fat(&mut self) -> io::Result<()> {
        if self.mini_fat_table.is_some() {
            return Ok(());
        }
        if self.mini_fat_sector_count == 0 || self.first_mini_fat_sector == END_OF_CHAIN {
            self.mini_fat_table = Some(Vec::new());
            return Ok(());
        }
        let bytes = read_chain_static(
            &self.data,
            self.sector_size,
            &self.fat_table,
            self.first_mini_fat_sector,
            (self.mini_fat_sector_count as u64) * (self.sector_size as u64),
        )?;
        let entries = bytes.len() / 4;
        let mut mft = Vec::with_capacity(entries);
        for i in 0..entries {
            mft.push(read_u32_le(&bytes, i * 4));
        }
        self.mini_fat_table = Some(mft);
        Ok(())
    }

    fn ensure_mini_stream(&mut self) -> io::Result<()> {
        if self.mini_stream.is_some() {
            return Ok(());
        }
        let root = self
            .dir_entries
            .first()
            .filter(|e| e.entry_type == TYPE_ROOT);
        let Some(root) = root else {
            self.mini_stream = Some(Vec::new());
            return Ok(());
        };
        let size = if root.size == 0 {
            MAX_STREAM_SIZE as u64
        } else {
            root.size
        };
        let ms = read_chain_static(
            &self.data,
            self.sector_size,
            &self.fat_table,
            root.start_sector,
            size,
        )?;
        self.mini_stream = Some(ms);
        Ok(())
    }

    fn read_mini_stream(
        &mut self,
        start_sector: u32,
        size: u64,
    ) -> io::Result<Vec<u8>> {
        self.ensure_mini_fat()?;
        self.ensure_mini_stream()?;

        let mft = self.mini_fat_table.as_ref().unwrap();
        let ms = self.mini_stream.as_ref().unwrap();
        if mft.is_empty() || ms.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        let mut current = start_sector;
        let mut total_read = 0u64;
        let mut visited_len = 0usize;
        let mut visited = Vec::new();

        while current != END_OF_CHAIN && current != FREE_SECT && total_read < size {
            if visited.contains(&current) {
                break;
            }
            if visited_len > MAX_CHAIN_LENGTH {
                break;
            }
            visited.push(current);
            visited_len += 1;

            let off = (current as usize) * self.mini_sector_size;
            let remaining = (size - total_read) as usize;
            let chunk_size = self.mini_sector_size.min(remaining);
            if off + chunk_size <= ms.len() {
                out.extend_from_slice(&ms[off..off + chunk_size]);
            }
            total_read += chunk_size as u64;

            current = if (current as usize) < mft.len() {
                mft[current as usize]
            } else {
                END_OF_CHAIN
            };
        }
        Ok(out)
    }
}

// ── Free helpers ─────────────────────────────────────────────────────────────

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn sector_offset(id: u32, sector_size: usize) -> usize {
    512 + (id as usize) * sector_size
}

fn read_sector_slice<'a>(
    data: &'a [u8],
    id: u32,
    sector_size: usize,
) -> &'a [u8] {
    let off = sector_offset(id, sector_size);
    if off + sector_size > data.len() {
        &[]
    } else {
        &data[off..off + sector_size]
    }
}

fn build_fat_sector_list(
    data: &[u8],
    sector_size: usize,
    fat_sector_count: usize,
    first_difat_sector: u32,
    difat_sector_count: usize,
) -> Vec<u32> {
    let mut fat_sectors = Vec::with_capacity(fat_sector_count);

    // Up to 109 entries directly in the header at offset 76.
    for i in 0..109 {
        if fat_sectors.len() >= fat_sector_count {
            break;
        }
        let sid = read_u32_le(data, 76 + i * 4);
        if sid == FREE_SECT || sid == END_OF_CHAIN {
            break;
        }
        fat_sectors.push(sid);
    }

    // Additional DIFAT sector chain.
    let mut difat_sector = first_difat_sector;
    let mut visited = Vec::new();
    let entries_per_sector = (sector_size / 4) - 1; // last u32 = next pointer

    for _ in 0..difat_sector_count {
        if difat_sector == END_OF_CHAIN || difat_sector == FREE_SECT {
            break;
        }
        if visited.contains(&difat_sector) {
            break;
        }
        visited.push(difat_sector);

        let buf = read_sector_slice(data, difat_sector, sector_size);
        if buf.is_empty() {
            break;
        }
        for i in 0..entries_per_sector {
            if fat_sectors.len() >= fat_sector_count {
                break;
            }
            if i * 4 + 3 >= buf.len() {
                break;
            }
            let sid = read_u32_le(buf, i * 4);
            if sid == FREE_SECT || sid == END_OF_CHAIN {
                continue;
            }
            fat_sectors.push(sid);
        }
        let next_off = entries_per_sector * 4;
        if next_off + 3 >= buf.len() {
            break;
        }
        difat_sector = read_u32_le(buf, next_off);
    }

    fat_sectors
}

fn build_fat_table(
    data: &[u8],
    sector_size: usize,
    fat_sectors: &[u32],
    entries_per_fat_sector: usize,
) -> Vec<u32> {
    let mut fat_table = Vec::with_capacity(fat_sectors.len() * entries_per_fat_sector);
    for sid in fat_sectors {
        let buf = read_sector_slice(data, *sid, sector_size);
        for i in 0..entries_per_fat_sector {
            let value = if i * 4 + 3 < buf.len() {
                read_u32_le(buf, i * 4)
            } else {
                FREE_SECT
            };
            fat_table.push(value);
        }
    }
    fat_table
}

fn read_chain_static(
    data: &[u8],
    sector_size: usize,
    fat_table: &[u32],
    start_sector: u32,
    max_bytes: u64,
) -> io::Result<Vec<u8>> {
    if start_sector == END_OF_CHAIN || start_sector == FREE_SECT {
        return Ok(Vec::new());
    }
    if max_bytes > MAX_STREAM_SIZE as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("chain exceeds {} bytes", MAX_STREAM_SIZE),
        ));
    }

    let mut out = Vec::new();
    let mut current = start_sector;
    let mut total_read = 0u64;
    let mut visited = Vec::new();

    while current != END_OF_CHAIN && current != FREE_SECT && total_read < max_bytes {
        if visited.contains(&current) {
            break;
        }
        if visited.len() > MAX_CHAIN_LENGTH {
            break;
        }
        visited.push(current);

        let buf = read_sector_slice(data, current, sector_size);
        let remaining = max_bytes - total_read;
        let take = (buf.len() as u64).min(remaining) as usize;
        out.extend_from_slice(&buf[..take]);
        total_read += take as u64;

        current = if (current as usize) < fat_table.len() {
            fat_table[current as usize]
        } else {
            END_OF_CHAIN
        };
    }

    Ok(out)
}

fn parse_dir_entries(dir_data: &[u8]) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    let mut offset = 0;
    while offset + 128 <= dir_data.len() && entries.len() < MAX_DIR_ENTRIES {
        let name_len = read_u16_le(dir_data, offset + 64); // byte count incl. NUL
        if name_len == 0 || name_len > 64 {
            entries.push(DirEntry {
                name: String::new(),
                entry_type: 0,
                start_sector: 0,
                size: 0,
            });
            offset += 128;
            continue;
        }
        let name_bytes = (name_len as usize).saturating_sub(2); // drop NUL
        let name = if name_bytes > 0 && offset + name_bytes <= dir_data.len() {
            utf16le_to_string(&dir_data[offset..offset + name_bytes])
        } else {
            String::new()
        };
        let entry_type = dir_data[offset + 66];
        let start_sector = read_u32_le(dir_data, offset + 116);
        // CFB v3 stores size as u32 at offset 120. v4 uses u64 but HWP is v3.
        let size = read_u32_le(dir_data, offset + 120) as u64;

        entries.push(DirEntry {
            name,
            entry_type,
            start_sector,
            size,
        });
        offset += 128;
    }
    entries
}

fn utf16le_to_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let code = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if code == 0 {
            break;
        }
        if let Some(c) = char::from_u32(code as u32) {
            out.push(c);
        }
        i += 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_input() {
        let err = LenientCfb::parse(vec![0u8; 100]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = vec![0u8; 512];
        data[..8].copy_from_slice(b"NOT_CFB!");
        let err = LenientCfb::parse(data).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }

    #[test]
    fn rejects_invalid_sector_shift() {
        let mut data = vec![0u8; 512];
        data[..8].copy_from_slice(&CFB_MAGIC);
        // sector_size_shift = 4 (too small; min is 7)
        data[30..32].copy_from_slice(&4u16.to_le_bytes());
        let err = LenientCfb::parse(data).unwrap_err();
        assert!(err.to_string().contains("sector size shift"));
    }

    #[test]
    fn accepts_minimal_empty_cfb() {
        // A synthetic empty CFB header — no FAT sectors, no dir. Should parse
        // successfully but produce 0 entries.
        let mut data = vec![0u8; 512];
        data[..8].copy_from_slice(&CFB_MAGIC);
        data[30..32].copy_from_slice(&9u16.to_le_bytes()); // 512 byte sectors
        data[32..34].copy_from_slice(&6u16.to_le_bytes()); // 64 byte mini sectors
        data[44..48].copy_from_slice(&0u32.to_le_bytes()); // 0 FAT sectors
        data[48..52].copy_from_slice(&END_OF_CHAIN.to_le_bytes()); // no dir
        data[56..60].copy_from_slice(&4096u32.to_le_bytes());
        data[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        data[64..68].copy_from_slice(&0u32.to_le_bytes());
        data[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        data[72..76].copy_from_slice(&0u32.to_le_bytes());
        let cfb = LenientCfb::parse(data).expect("minimal header should parse");
        assert_eq!(cfb.entries().len(), 0);
        assert_eq!(cfb.stream_count_with_prefix("BodyText/"), 0);
    }
}
