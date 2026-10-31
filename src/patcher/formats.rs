//! NGDP wire formats. See wowdev.wiki/TACT and wowdev.wiki/BLTE.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
};
pub type Key = [u8; 16];
pub type Result<T> = std::result::Result<T, String>;
pub fn hash(bytes: &[u8]) -> Key {
    md5::compute(bytes).0
}
pub fn hex(key: &[u8]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn key(text: &str) -> Result<Key> {
    if text.len() != 32 || !text.is_ascii() {
        return Err("Invalid NGDP content key.".into());
    }
    let mut out = [0; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| "Invalid NGDP content key.")?;
    }
    Ok(out)
}
pub struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("Manifest size overflow.")?;
        let b = self
            .bytes
            .get(self.pos..end)
            .ok_or("Truncated Blizzard manifest.")?;
        self.pos = end;
        Ok(b)
    }
    pub fn be(&mut self, n: usize) -> Result<u64> {
        if n > 8 {
            return Err("Invalid manifest integer size.".into());
        }
        Ok(self
            .take(n)?
            .iter()
            .fold(0, |v, b| (v << 8) | u64::from(*b)))
    }
    pub fn string(&mut self) -> Result<String> {
        let tail = self
            .bytes
            .get(self.pos..)
            .ok_or("Invalid manifest string.")?;
        let n = tail
            .iter()
            .position(|b| *b == 0)
            .ok_or("Unterminated manifest string.")?;
        if n > 4096 {
            return Err("Manifest string too long.".into());
        }
        let s = std::str::from_utf8(self.take(n)?)
            .map_err(|_| "Invalid manifest string.")?
            .to_owned();
        self.take(1)?;
        Ok(s)
    }
}
pub fn table(text: &str) -> Result<Vec<HashMap<String, String>>> {
    let mut lines = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty());
    let columns: Vec<_> = lines
        .next()
        .ok_or("Empty Blizzard table.")?
        .split('|')
        .map(|s| s.split('!').next().unwrap().to_owned())
        .collect();
    Ok(lines
        .map(|line| {
            columns
                .iter()
                .cloned()
                .zip(line.split('|').map(str::to_owned))
                .collect()
        })
        .collect())
}
pub fn config(bytes: &[u8]) -> Result<HashMap<String, Vec<String>>> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Invalid Blizzard configuration.")?;
    Ok(text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| {
            (
                k.trim().to_owned(),
                v.split_whitespace().map(str::to_owned).collect(),
            )
        })
        .collect())
}
pub fn blte(blob: &[u8], expected: Key) -> Result<Vec<u8>> {
    let mut r = Reader::new(blob);
    if r.take(4)? != b"BLTE" {
        return Err("Invalid BLTE header.".into());
    }
    let header = r.be(4)? as usize;
    if header == 0 {
        if hash(blob) != expected {
            return Err("BLTE content checksum mismatch.".into());
        }
        return decode_chunk(r.take(blob.len() - 8)?, 0);
    }
    if header < 12 || header > blob.len() || hash(&blob[..header]) != expected {
        return Err("BLTE header checksum mismatch.".into());
    }
    if r.be(1)? != 15 {
        return Err("Unsupported BLTE flags.".into());
    }
    let count = r.be(3)? as usize;
    if count > 1_000_000 || 12usize.checked_add(count * 24) != Some(header) {
        return Err("Invalid BLTE chunk table.".into());
    }
    let mut offset = header;
    let mut out = Vec::new();
    for _ in 0..count {
        let compressed = r.be(4)? as usize;
        let decoded = r.be(4)? as usize;
        let checksum = r.take(16)?;
        let end = offset
            .checked_add(compressed)
            .ok_or("BLTE size overflow.")?;
        let chunk = blob.get(offset..end).ok_or("Truncated BLTE chunk.")?;
        if hash(chunk).as_slice() != checksum {
            return Err("BLTE chunk checksum mismatch.".into());
        }
        if out
            .len()
            .checked_add(decoded)
            .is_none_or(|n| n > 1024 * 1024 * 1024)
        {
            return Err("BLTE decoded file exceeds supported size.".into());
        }
        let bytes = decode_chunk(chunk, decoded)?;
        if bytes.len() != decoded {
            return Err("BLTE decoded size mismatch.".into());
        }
        out.extend(bytes);
        offset = end;
    }
    if offset != blob.len() {
        return Err("Unexpected bytes after BLTE chunks.".into());
    }
    Ok(out)
}
fn decode_chunk(chunk: &[u8], expected: usize) -> Result<Vec<u8>> {
    let (&mode, body) = chunk.split_first().ok_or("Empty BLTE chunk.")?;
    let limit = if expected == 0 {
        1024 * 1024 * 1024
    } else {
        expected
    };
    match mode {
        b'N' => {
            if body.len() > limit {
                return Err("BLTE chunk exceeds expected size.".into());
            }
            Ok(body.to_vec())
        }
        b'Z' => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(body)
                .take(limit as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|e| e.to_string())?;
            if out.len() > limit {
                return Err("BLTE decompression exceeded expected size.".into());
            }
            Ok(out)
        }
        b'E' => {
            Err("This patch contains encrypted content that needs an unsupported game key.".into())
        }
        _ => Err("Unsupported BLTE compression mode.".into()),
    }
}
#[derive(Clone)]
pub struct Tag {
    pub name: String,
    pub kind: u16,
    pub mask: Vec<u8>,
}
fn tags(r: &mut Reader, count: usize, entries: usize) -> Result<Vec<Tag>> {
    if count > 4096 || entries > 10_000_000 {
        return Err("Blizzard manifest exceeds supported size.".into());
    }
    (0..count)
        .map(|_| {
            Ok(Tag {
                name: r.string()?,
                kind: r.be(2)? as u16,
                mask: r.take(entries.div_ceil(8))?.to_vec(),
            })
        })
        .collect()
}
fn selection(tags: &[Tag], count: usize, spec: &str) -> Vec<bool> {
    let mut result = vec![false; count];
    for set in spec.split(':') {
        let names: HashSet<_> = set
            .split_whitespace()
            .map(|s| s.trim_end_matches('?'))
            .collect();
        let mut groups: BTreeMap<u16, Vec<&Tag>> = BTreeMap::new();
        for tag in tags {
            if tag.kind & 0x4000 == 0 && names.contains(tag.name.as_str()) {
                groups.entry(tag.kind).or_default().push(tag);
            }
        }
        for (i, selected) in result.iter_mut().enumerate() {
            *selected |= groups.values().all(|group| {
                group
                    .iter()
                    .any(|tag| tag.mask[i / 8] & (0x80 >> (i % 8)) != 0)
            });
        }
    }
    result
}
#[derive(Clone, Debug)]
pub struct InstallFile {
    pub path: String,
    pub content: Key,
    pub size: u64,
}
pub fn install(bytes: &[u8], spec: &str) -> Result<Vec<InstallFile>> {
    let mut r = Reader::new(bytes);
    if r.take(2)? != b"IN" || r.be(1)? != 1 || r.be(1)? != 16 {
        return Err("Unsupported install manifest.".into());
    }
    let tag_count = r.be(2)? as usize;
    let count = r.be(4)? as usize;
    let tags = tags(&mut r, tag_count, count)?;
    let selected = selection(&tags, count, spec);
    let mut files = Vec::new();
    for take in selected {
        let path = r.string()?.replace('\\', "/");
        let content = r.take(16)?.try_into().unwrap();
        let size = r.be(4)?;
        if take {
            files.push(InstallFile {
                path,
                content,
                size,
            });
        }
    }
    Ok(files)
}
pub fn download(bytes: &[u8], spec: &str) -> Result<Vec<(Key, u64)>> {
    let mut r = Reader::new(bytes);
    if r.take(2)? != b"DL" {
        return Err("Invalid download manifest.".into());
    }
    let version = r.be(1)?;
    if !(1..=3).contains(&version) || r.be(1)? != 16 {
        return Err("Unsupported download manifest.".into());
    }
    let checksum = r.be(1)? != 0;
    let count = r.be(4)? as usize;
    let tag_count = r.be(2)? as usize;
    if count > 10_000_000 {
        return Err("Download manifest exceeds supported size.".into());
    }
    let flags = if version >= 2 { r.be(1)? as usize } else { 0 };
    if version >= 3 {
        r.take(4)?;
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let key = r.take(16)?.try_into().unwrap();
        let size = r.be(5)?;
        r.take(1 + if checksum { 4 } else { 0 } + flags)?;
        entries.push((key, size));
    }
    let tags = tags(&mut r, tag_count, count)?;
    let selected = selection(&tags, count, spec);
    Ok(entries
        .into_iter()
        .zip(selected)
        .filter_map(|(entry, selected)| selected.then_some(entry))
        .collect())
}
pub fn encoding(bytes: &[u8], wanted: &HashSet<Key>) -> Result<HashMap<Key, Key>> {
    let mut r = Reader::new(bytes);
    if r.take(2)? != b"EN" || r.be(1)? != 1 || r.be(1)? != 16 || r.be(1)? != 16 {
        return Err("Unsupported encoding table.".into());
    }
    let page = r.be(2)? as usize * 1024;
    r.take(2)?;
    let count = r.be(4)? as usize;
    r.take(5)?;
    let strings = r.be(4)? as usize;
    let start = r
        .pos
        .checked_add(strings)
        .and_then(|n| n.checked_add(count.checked_mul(32)?))
        .ok_or("Encoding table size overflow.")?;
    if page == 0
        || count > 1_000_000
        || start
            .checked_add(count * page)
            .is_none_or(|n| n > bytes.len())
    {
        return Err("Invalid encoding table pages.".into());
    }
    let mut found = HashMap::new();
    for p in 0..count {
        let mut r = Reader::new(&bytes[start + p * page..start + (p + 1) * page]);
        while r.pos + 38 <= page {
            let n = r.be(1)? as usize;
            if n == 0 {
                break;
            }
            r.take(5)?;
            let content: Key = r.take(16)?.try_into().unwrap();
            let first: Key = r.take(16)?.try_into().unwrap();
            r.take((n - 1) * 16)?;
            if wanted.contains(&content) {
                found.insert(content, first);
            }
        }
        if found.len() == wanted.len() {
            break;
        }
    }
    Ok(found)
}
#[derive(Clone, Debug)]
pub struct Location {
    pub archive: String,
    pub offset: u64,
    pub size: u64,
}
pub fn archive_index(
    bytes: &[u8],
    archive: &str,
    wanted: &HashSet<Key>,
) -> Result<HashMap<Key, Location>> {
    if bytes.len() < 28 {
        return Err("Truncated CDN index.".into());
    }
    let f = &bytes[bytes.len() - 20..];
    let block = f[3] as usize * 1024;
    let offset = f[4] as usize;
    let size = f[5] as usize;
    let key_size = f[6] as usize;
    let checksum = f[7] as usize;
    if block == 0 || key_size != 16 || offset > 8 || size > 8 || offset == 0 || size == 0 {
        return Err("Unsupported CDN index format.".into());
    }
    let count = (bytes.len() - 28) / (block + key_size + checksum);
    let mut found = HashMap::new();
    for i in 0..count {
        let mut r = Reader::new(&bytes[i * block..(i + 1) * block]);
        while r.pos + key_size + size + offset <= block {
            let key: Key = r.take(16)?.try_into().unwrap();
            if key == [0; 16] {
                break;
            }
            let size = r.be(size)?;
            let offset = r.be(offset)?;
            if wanted.contains(&key) {
                found.insert(
                    key,
                    Location {
                        archive: archive.into(),
                        size,
                        offset,
                    },
                );
            }
        }
    }
    Ok(found)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_corrupted_blte_before_decoding() {
        let blob = b"BLTE\0\0\0\0Nhello";
        assert_eq!(blte(blob, hash(blob)).unwrap(), b"hello");
        assert!(blte(blob, [0; 16]).is_err());
        assert!(blte(b"BLTE", [0; 16]).is_err());
    }
    #[test]
    fn tag_groups_require_each_type_but_accept_alternatives() {
        let tags = vec![
            Tag {
                name: "Windows".into(),
                kind: 1,
                mask: vec![0xc0],
            },
            Tag {
                name: "OSX".into(),
                kind: 1,
                mask: vec![0x20],
            },
            Tag {
                name: "enUS".into(),
                kind: 2,
                mask: vec![0xa0],
            },
            Tag {
                name: "code".into(),
                kind: 0x4001,
                mask: vec![0],
            },
        ];
        assert_eq!(
            selection(&tags, 3, "Windows enUS code"),
            vec![true, false, false]
        );
        assert_eq!(
            selection(&tags, 3, "Windows enUS:OSX enUS"),
            vec![true, false, true]
        );
    }
}
/// Validate an encoded CDN file while keeping memory bounded by its chunk table.
pub fn verify_file(path: &std::path::Path, expected: Key) -> Result<()> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    verify_reader(&mut file, length, expected)
}
pub fn verify_reader(mut file: impl std::io::Read, length: u64, expected: Key) -> Result<()> {
    if length > 1024 * 1024 * 1024 {
        return Err("Encoded game file exceeds CASC archive size.".into());
    }
    let mut first = [0; 8];
    file.read_exact(&mut first).map_err(|e| e.to_string())?;
    if &first[..4] != b"BLTE" {
        return Err("Invalid downloaded BLTE file.".into());
    }
    let size = u32::from_be_bytes(first[4..].try_into().unwrap()) as usize;
    let mut buffer = [0; 65536];
    if size == 0 {
        let mut context = md5::Context::new();
        context.consume(first);
        loop {
            let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            context.consume(&buffer[..n]);
        }
        if context.finalize().0 != expected {
            return Err("Downloaded game file checksum failed.".into());
        }
        return Ok(());
    }
    if !(12..=64 * 1024 * 1024).contains(&size) || size as u64 > length {
        return Err("Invalid BLTE chunk-table size.".into());
    }
    let mut header = first.to_vec();
    header.resize(size, 0);
    file.read_exact(&mut header[8..])
        .map_err(|e| e.to_string())?;
    if hash(&header) != expected {
        return Err("Downloaded game-file header checksum failed.".into());
    }
    let mut r = Reader::new(&header[8..]);
    if r.be(1)? != 15 {
        return Err("Unsupported BLTE flags.".into());
    }
    let count = r.be(3)? as usize;
    if 12 + count * 24 != size {
        return Err("Invalid BLTE chunk count.".into());
    }
    let mut total = size as u64;
    for _ in 0..count {
        let mut remaining = r.be(4)?;
        r.take(4)?;
        let expected = r.take(16)?;
        let mut context = md5::Context::new();
        total = total
            .checked_add(remaining)
            .ok_or("BLTE length overflow.")?;
        while remaining > 0 {
            let n = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..n])
                .map_err(|e| e.to_string())?;
            context.consume(&buffer[..n]);
            remaining -= n as u64;
        }
        if context.finalize().0.as_slice() != expected {
            return Err("Downloaded game-file chunk checksum failed.".into());
        }
    }
    if total != length {
        return Err("Downloaded game-file size mismatch.".into());
    }
    Ok(())
}
