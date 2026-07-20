// Ported from kkdoc (MIT): src/roundtrip/zip-patch.ts
//! In-place ZIP patch — replace only the named entries; every other entry's
//! local record bytes are copied verbatim (only offset fields are patched).
//!
//! The `zip` crate's writer re-serializes/re-compresses every entry, which
//! breaks byte preservation and can reorder the mandatory first `mimetype`
//! STORED entry (OCF / HWPX rule). Here we parse the Central Directory directly,
//! rewrite only changed local records, and copy the rest — so the mimetype-first
//! ordering and all untouched formatting survive automatically.

use std::collections::HashMap;
use std::io::Write;

use flate2::write::DeflateEncoder;
use flate2::Compression;

const EOCD_SIG: u32 = 0x0605_4b50;
const CD_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const ZIP64_EOCD_LOC_SIG: u32 = 0x0706_4b50;

#[derive(Debug, Clone)]
struct CdEntry {
    cd_start: usize,
    cd_end: usize,
    name: String,
    flags: u16,
    method: u16,
    comp_size: u32,
    uncomp_size: u32,
    local_offset: u32,
}

#[inline]
fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = flate2::Crc::new();
    c.update(data);
    c.sum()
}

fn deflate_raw(data: &[u8]) -> Vec<u8> {
    let mut e = DeflateEncoder::new(Vec::new(), Compression::new(6));
    e.write_all(data).expect("deflate write");
    e.finish().expect("deflate finish")
}

struct ParsedCd {
    entries: Vec<CdEntry>,
    cd_offset: usize,
    eocd_offset: usize,
}

fn parse_central_directory(buf: &[u8]) -> Result<ParsedCd, String> {
    if buf.len() < 22 {
        return Err("ZIP too small".into());
    }
    let min_eocd = buf.len().saturating_sub(22 + 65535);
    let mut eocd_offset: i64 = -1;
    let mut i = buf.len() - 22;
    loop {
        if u32le(buf, i) == EOCD_SIG
            && i + 22 + u16le(buf, i + 20) as usize == buf.len()
        {
            eocd_offset = i as i64;
            break;
        }
        if i == min_eocd {
            break;
        }
        i -= 1;
    }
    if eocd_offset < 0 {
        // fallback: trailing junk after EOCD
        let mut j = buf.len() - 22;
        loop {
            if u32le(buf, j) == EOCD_SIG {
                let cand = u32le(buf, j + 16) as usize;
                if j + 22 + u16le(buf, j + 20) as usize <= buf.len()
                    && cand + 4 < buf.len()
                    && u32le(buf, cand) == CD_SIG
                {
                    eocd_offset = j as i64;
                    break;
                }
            }
            if j == min_eocd {
                break;
            }
            j -= 1;
        }
    }
    if eocd_offset < 0 {
        return Err("ZIP EOCD를 찾을 수 없습니다".into());
    }
    let eocd_offset = eocd_offset as usize;
    let total = u16le(buf, eocd_offset + 10);
    let cd_offset = u32le(buf, eocd_offset + 16);
    if cd_offset == 0xffff_ffff || total == 0xffff {
        return Err("ZIP64는 지원하지 않습니다".into());
    }
    if eocd_offset >= 20 && u32le(buf, eocd_offset - 20) == ZIP64_EOCD_LOC_SIG {
        return Err("ZIP64는 지원하지 않습니다".into());
    }
    let mut entries = Vec::with_capacity(total as usize);
    let mut pos = cd_offset as usize;
    for _ in 0..total {
        if u32le(buf, pos) != CD_SIG {
            return Err("ZIP Central Directory 손상".into());
        }
        let flags = u16le(buf, pos + 8);
        let method = u16le(buf, pos + 10);
        let comp_size = u32le(buf, pos + 20);
        let uncomp_size = u32le(buf, pos + 24);
        let name_len = u16le(buf, pos + 28) as usize;
        let extra_len = u16le(buf, pos + 30) as usize;
        let comment_len = u16le(buf, pos + 32) as usize;
        let local_offset = u32le(buf, pos + 42);
        if comp_size == 0xffff_ffff || uncomp_size == 0xffff_ffff || local_offset == 0xffff_ffff {
            return Err("ZIP64는 지원하지 않습니다".into());
        }
        let name = String::from_utf8_lossy(&buf[pos + 46..pos + 46 + name_len]).into_owned();
        let cd_end = pos + 46 + name_len + extra_len + comment_len;
        entries.push(CdEntry {
            cd_start: pos,
            cd_end,
            name,
            flags,
            method,
            comp_size,
            uncomp_size,
            local_offset,
        });
        pos = cd_end;
    }
    Ok(ParsedCd { entries, cd_offset: cd_offset as usize, eocd_offset })
}

fn local_data_start(buf: &[u8], local_offset: u32) -> Result<usize, String> {
    let lo = local_offset as usize;
    if u32le(buf, lo) != LOCAL_SIG {
        return Err("ZIP 로컬 헤더 시그니처 불일치".into());
    }
    let name_len = u16le(buf, lo + 26) as usize;
    let extra_len = u16le(buf, lo + 28) as usize;
    Ok(lo + 30 + name_len + extra_len)
}

/// Read each entry's raw (still-compressed) data — for verification/tests.
pub fn read_zip_entries(buf: &[u8]) -> Result<HashMap<String, (u16, Vec<u8>)>, String> {
    let parsed = parse_central_directory(buf)?;
    let mut out = HashMap::new();
    for e in &parsed.entries {
        let ds = local_data_start(buf, e.local_offset)?;
        out.insert(e.name.clone(), (e.method, buf[ds..ds + e.comp_size as usize].to_vec()));
    }
    Ok(out)
}

/// Replace the named entries with new (uncompressed) data, copying every other
/// entry's local record bytes verbatim. Entry order (mimetype-first) is preserved.
pub fn patch_zip_entries(
    original: &[u8],
    replacements: &HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let parsed = parse_central_directory(original)?;
    let entries = &parsed.entries;

    for name in replacements.keys() {
        if !entries.iter().any(|e| &e.name == name) {
            return Err(format!("ZIP에 없는 엔트리: {name}"));
        }
    }

    // local record order = original local-offset order
    let mut by_local: Vec<&CdEntry> = entries.iter().collect();
    by_local.sort_by_key(|e| e.local_offset);

    let mut segments: Vec<Vec<u8>> = Vec::new();
    let mut new_local_offset: HashMap<usize, u32> = HashMap::new(); // cd_start -> offset
    let mut new_meta: HashMap<usize, (u32, u32, u32, u16)> = HashMap::new(); // crc, comp, uncomp, flags
    let mut offset: u32 = 0;

    for i in 0..by_local.len() {
        let e = by_local[i];
        let seg_end = if i + 1 < by_local.len() {
            by_local[i + 1].local_offset as usize
        } else {
            parsed.cd_offset
        };
        new_local_offset.insert(e.cd_start, offset);

        match replacements.get(&e.name) {
            None => {
                let seg = original[e.local_offset as usize..seg_end].to_vec();
                offset += seg.len() as u32;
                segments.push(seg);
            }
            Some(new_data) => {
                let lo = e.local_offset as usize;
                if u32le(original, lo) != LOCAL_SIG {
                    return Err("ZIP 로컬 헤더 시그니처 불일치".into());
                }
                let name_len = u16le(original, lo + 26) as usize;
                let extra_len = u16le(original, lo + 28) as usize;
                let header_len = 30 + name_len + extra_len;
                let mut header = original[lo..lo + header_len].to_vec();

                // Reject encrypted target entries (general purpose bit 0).
                // Replacing an encrypted entry's payload with plaintext DEFLATE while
                // keeping the encryption flag makes the reader attempt decryption and
                // judge the HWPX corrupt. Untouched encrypted entries pass through
                // byte-preserved (the flag stays valid) — only patched ones are refused.
                if e.flags & 0x0001 != 0 {
                    return Err(format!(
                        "암호화된 ZIP 엔트리는 패치할 수 없습니다: {} — 평문 교체 시 리더가 복호화 실패로 파손 판정",
                        e.name
                    ));
                }
                if e.method != 0 && e.method != 8 {
                    return Err(format!(
                        "지원하지 않는 ZIP 압축 방식(method={}): {} — STORE(0)/DEFLATE(8)만 교체 가능",
                        e.method, e.name
                    ));
                }
                let comp_data = if e.method == 0 { new_data.clone() } else { deflate_raw(new_data) };
                let crc = crc32(new_data);
                let flags = e.flags & !0x0008; // clear data-descriptor bit

                header[6..8].copy_from_slice(&flags.to_le_bytes());
                header[14..18].copy_from_slice(&crc.to_le_bytes());
                header[18..22].copy_from_slice(&(comp_data.len() as u32).to_le_bytes());
                header[22..26].copy_from_slice(&(new_data.len() as u32).to_le_bytes());

                offset += (header_len + comp_data.len()) as u32;
                segments.push(header);
                segments.push(comp_data.clone());
                new_meta.insert(e.cd_start, (crc, comp_data.len() as u32, new_data.len() as u32, flags));
            }
        }
    }

    // central directory — original order, patch offsets/meta
    let new_cd_offset = offset;
    for e in entries {
        let mut cd = original[e.cd_start..e.cd_end].to_vec();
        let off = *new_local_offset.get(&e.cd_start).unwrap();
        cd[42..46].copy_from_slice(&off.to_le_bytes());
        if let Some((crc, comp, uncomp, flags)) = new_meta.get(&e.cd_start) {
            cd[8..10].copy_from_slice(&flags.to_le_bytes());
            cd[16..20].copy_from_slice(&crc.to_le_bytes());
            cd[20..24].copy_from_slice(&comp.to_le_bytes());
            cd[24..28].copy_from_slice(&uncomp.to_le_bytes());
        }
        offset += cd.len() as u32;
        segments.push(cd);
    }
    let new_cd_size = offset - new_cd_offset;

    // EOCD — copy original, patch CD offset/size
    let mut eocd = original[parsed.eocd_offset..].to_vec();
    eocd[12..16].copy_from_slice(&new_cd_size.to_le_bytes());
    eocd[16..20].copy_from_slice(&new_cd_offset.to_le_bytes());
    segments.push(eocd);

    let total: usize = segments.iter().map(|s| s.len()).sum();
    let mut result = Vec::with_capacity(total);
    for seg in segments {
        result.extend_from_slice(&seg);
    }
    Ok(result)
}
