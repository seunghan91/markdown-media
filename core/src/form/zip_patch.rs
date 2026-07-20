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

/// Like [`patch_zip_entries`] but also appends brand-new entries (e.g. a seal
/// image `BinData/imageN.png`) — used when a feature must add binary parts to
/// an HWPX/OWPML zip while every existing entry stays byte-identical. New
/// entries are stored uncompressed (method 0) and appended after the last
/// existing local record, before the Central Directory.
pub fn patch_zip_entries_with_additions(
    original: &[u8],
    replacements: &HashMap<String, Vec<u8>>,
    additions: &HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let parsed = parse_central_directory(original)?;
    let entries = &parsed.entries;

    for name in replacements.keys() {
        if !entries.iter().any(|e| &e.name == name) {
            return Err(format!("ZIP에 없는 엔트리: {name}"));
        }
    }
    for name in additions.keys() {
        if entries.iter().any(|e| &e.name == name) {
            return Err(format!("ZIP에 이미 존재하는 엔트리: {name}"));
        }
    }

    // EOCD의 엔트리 수 필드는 u16 — 0xffff는 "ZIP64 EOCD를 보라"는 예약 신호값이라,
    // 총 엔트리 수가 그 값에 닿거나(u16 캐스팅 시 랩어라운드로 더 조용히 깨짐)
    // 넘으면 이 함수가 쓰는 일반 EOCD로는 더 이상 올바르게 표현할 수 없다.
    // ZIP64 EOCD/locator를 쓰지 않으므로 여기서 명시적으로 거부한다.
    let total_entries = entries.len() + additions.len();
    if total_entries >= 0xffff {
        return Err(format!(
            "ZIP64는 지원하지 않습니다 — 추가 후 엔트리 수가 너무 많습니다 (기존 {} + 추가 {} = {} >= 65535)",
            entries.len(),
            additions.len(),
            total_entries
        ));
    }

    let mut by_local: Vec<&CdEntry> = entries.iter().collect();
    by_local.sort_by_key(|e| e.local_offset);

    let mut segments: Vec<Vec<u8>> = Vec::new();
    let mut new_local_offset: HashMap<usize, u32> = HashMap::new();
    let mut new_meta: HashMap<usize, (u32, u32, u32, u16)> = HashMap::new();
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
                let flags = e.flags & !0x0008;

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

    // brand-new local records — appended after the last existing one
    struct NewEntry {
        name: String,
        local_offset: u32,
        crc: u32,
        size: u32,
    }
    let mut new_entries: Vec<NewEntry> = Vec::new();
    for (name, data) in additions {
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        let local_offset = offset;
        let mut local = Vec::with_capacity(30 + name_bytes.len() + data.len());
        local.extend_from_slice(&LOCAL_SIG.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes()); // version needed
        local.extend_from_slice(&0u16.to_le_bytes()); // flags
        local.extend_from_slice(&0u16.to_le_bytes()); // method = STORE
        local.extend_from_slice(&0u16.to_le_bytes()); // modtime
        local.extend_from_slice(&0x21u16.to_le_bytes()); // moddate = 1980-01-01
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes()); // comp size
        local.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncomp size
        local.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes()); // extra len
        local.extend_from_slice(name_bytes);
        local.extend_from_slice(data);
        offset += local.len() as u32;
        segments.push(local);
        new_entries.push(NewEntry { name: name.clone(), local_offset, crc, size: data.len() as u32 });
    }

    // central directory — original entries (original order, patched offsets/meta)
    // followed by the new entries
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
    for ne in &new_entries {
        let name_bytes = ne.name.as_bytes();
        let mut cd = Vec::with_capacity(46 + name_bytes.len());
        cd.extend_from_slice(&CD_SIG.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes()); // version made by
        cd.extend_from_slice(&20u16.to_le_bytes()); // version needed
        cd.extend_from_slice(&0u16.to_le_bytes()); // flags
        cd.extend_from_slice(&0u16.to_le_bytes()); // method
        cd.extend_from_slice(&0u16.to_le_bytes()); // modtime
        cd.extend_from_slice(&0x21u16.to_le_bytes()); // moddate
        cd.extend_from_slice(&ne.crc.to_le_bytes());
        cd.extend_from_slice(&ne.size.to_le_bytes());
        cd.extend_from_slice(&ne.size.to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra len
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment len
        cd.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        cd.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        cd.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        cd.extend_from_slice(&ne.local_offset.to_le_bytes());
        cd.extend_from_slice(name_bytes);
        offset += cd.len() as u32;
        segments.push(cd);
    }
    let new_cd_size = offset - new_cd_offset;

    // EOCD — copy original, patch entry counts + CD offset/size
    let mut eocd = original[parsed.eocd_offset..].to_vec();
    let new_total = entries.len() as u16 + new_entries.len() as u16;
    eocd[8..10].copy_from_slice(&new_total.to_le_bytes());
    eocd[10..12].copy_from_slice(&new_total.to_le_bytes());
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

#[cfg(test)]
mod additions_tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn build_zip_with_entry_count(n: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for i in 0..n {
                zw.start_file(format!("f{i}"), stored).unwrap();
            }
            zw.finish().unwrap();
        }
        buf
    }

    #[test]
    fn additions_round_trip_preserves_existing_and_adds_new_entry() {
        let bytes = build_zip_with_entry_count(3);
        let mut additions = HashMap::new();
        additions.insert("BinData/image1.png".to_string(), vec![1u8, 2, 3, 4]);
        let out = patch_zip_entries_with_additions(&bytes, &HashMap::new(), &additions).unwrap();

        let before = read_zip_entries(&bytes).unwrap();
        let after = read_zip_entries(&out).unwrap();
        for (name, data) in &before {
            assert_eq!(after.get(name), Some(data), "existing entry {name} preserved");
        }
        assert_eq!(after.get("BinData/image1.png").unwrap().1, vec![1u8, 2, 3, 4]);
        assert_eq!(after.len(), before.len() + 1);
    }

    #[test]
    fn additions_rejects_when_total_entry_count_reaches_zip64_threshold() {
        // 기존 65,534개 + 신규 1개 = 65,535(0xffff) — EOCD 엔트리 수 필드가 표현할 수
        // 있는 한계(그리고 ZIP64 EOCD를 봐야 한다는 예약 신호값)에 정확히 닿는 경계.
        // 이 함수는 ZIP64 EOCD/locator를 쓰지 않으므로 명시적으로 거부해야 한다.
        let bytes = build_zip_with_entry_count(65_534);
        let mut additions = HashMap::new();
        additions.insert("BinData/image1.png".to_string(), b"x".to_vec());
        let err = patch_zip_entries_with_additions(&bytes, &HashMap::new(), &additions).unwrap_err();
        assert!(err.contains("ZIP64"), "got: {err}");
    }

    #[test]
    fn additions_allows_just_under_the_zip64_threshold() {
        // 기존 65,533개 + 신규 1개 = 65,534 (< 0xffff) — 경계 바로 아래는 통과해야 한다.
        let bytes = build_zip_with_entry_count(65_533);
        let mut additions = HashMap::new();
        additions.insert("BinData/image1.png".to_string(), b"x".to_vec());
        let out = patch_zip_entries_with_additions(&bytes, &HashMap::new(), &additions);
        assert!(out.is_ok(), "just under the threshold must be accepted: {out:?}");
    }
}
