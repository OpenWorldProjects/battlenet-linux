//! Append-only CASC updates. Existing data archives are never rewritten.
use super::formats::{Key, Result};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
const LIMIT: u64 = 1 << 30;
#[derive(Clone, Debug)]
pub struct Entry {
    pub bucket: u8,
    pub key: [u8; 9],
    pub location: u64,
    pub size: u32,
}
pub struct Storage {
    pub directory: PathBuf,
    pub entries: HashMap<[u8; 9], Entry>,
    versions: [u32; 16],
    base_key: Key,
    archive: u16,
    offset: u64,
    new_files: Vec<PathBuf>,
    index_committed: bool,
}
fn le(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("Truncated CASC index.")?
            .try_into()
            .unwrap(),
    ))
}
fn be(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0, |v, b| (v << 8) | u64::from(*b))
}
fn bucket(key: &[u8]) -> usize {
    let b = key[..9].iter().fold(0, |v, b| v ^ b);
    ((b ^ (b >> 4)) & 15) as usize
}
// Bob Jenkins' public-domain lookup3/hashlittle2 algorithm.
fn lookup(bytes: &[u8], pc: u32, pb: u32) -> (u32, u32) {
    let mut a = 0xdeadbeefu32
        .wrapping_add(bytes.len() as u32)
        .wrapping_add(pc);
    let mut b = a;
    let mut c = a.wrapping_add(pb);
    let mut rest = bytes;
    while rest.len() > 12 {
        a = a.wrapping_add(u32::from_le_bytes(rest[..4].try_into().unwrap()));
        b = b.wrapping_add(u32::from_le_bytes(rest[4..8].try_into().unwrap()));
        c = c.wrapping_add(u32::from_le_bytes(rest[8..12].try_into().unwrap()));
        a = a.wrapping_sub(c);
        a ^= c.rotate_left(4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= a.rotate_left(6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= b.rotate_left(8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= c.rotate_left(16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= a.rotate_left(19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= b.rotate_left(4);
        b = b.wrapping_add(a);
        rest = &rest[12..];
    }
    if rest.is_empty() {
        return (c, b);
    }
    for (i, byte) in rest.iter().enumerate() {
        let v = u32::from(*byte) << ((i % 4) * 8);
        match i / 4 {
            0 => a = a.wrapping_add(v),
            1 => b = b.wrapping_add(v),
            _ => c = c.wrapping_add(v),
        }
    }
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    (c, b)
}
fn record(bytes: &[u8], bucket: u8) -> Entry {
    Entry {
        bucket,
        key: bytes[..9].try_into().unwrap(),
        location: be(&bytes[9..14]),
        size: u32::from_le_bytes(bytes[14..18].try_into().unwrap()),
    }
}
fn index(bytes: &[u8]) -> Result<HashMap<[u8; 9], Entry>> {
    let n = le(bytes, 0)? as usize;
    if n != 16 || bytes.len() < 40 || bytes[8..10] != [7, 0] || bytes[12..16] != [4, 5, 9, 30] {
        return Err("Unsupported local CASC index layout.".into());
    }
    if lookup(&bytes[8..8 + n], 0, 0).0 != le(bytes, 4)? {
        return Err("Local CASC index header checksum failed.".into());
    }
    let start = (8 + n + 15) & !15;
    let size = le(bytes, start)? as usize;
    let body = bytes
        .get(start + 8..start + 8 + size)
        .ok_or("Truncated CASC index body.")?;
    if size % 18 != 0 {
        return Err("Invalid CASC index record size.".into());
    }
    let bytes_bucket = bytes[10];
    let mut entries = HashMap::new();
    let (mut pc, mut pb) = (0, 0);
    for bytes in body.chunks_exact(18) {
        (pc, pb) = lookup(bytes, pc, pb);
        let e = record(bytes, bytes_bucket);
        entries.insert(e.key, e);
    }
    if pc != le(bytes, start + 4)? {
        return Err("Local CASC index content checksum failed.".into());
    }
    let mut page = (start + 8 + size + 511) & !511;
    while page + 512 <= bytes.len() {
        for slot in 0..21 {
            let at = page + slot * 24;
            let guard = le(bytes, at)?;
            if guard == 0 {
                break;
            }
            let entry = &bytes[at..at + 24];
            if lookup(&entry[4..23], 0, 0).0 | 0x80000000 != guard {
                return Err("Local CASC journal checksum failed.".into());
            }
            let e = record(&entry[4..22], bytes_bucket);
            if entry[22] == 0 {
                entries.insert(e.key, e);
            } else {
                entries.remove(&e.key);
            }
        }
        page += 512;
    }
    Ok(entries)
}
fn serialized_index(bucket: usize, entries: impl Iterator<Item = Entry>) -> Vec<u8> {
    let mut header = vec![7, 0, bucket as u8, 0, 4, 5, 9, 30];
    header.extend((1023u64 << 30).to_le_bytes());
    let mut bytes = Vec::new();
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(lookup(&header, 0, 0).0.to_le_bytes());
    bytes.extend(header);
    bytes.resize(32, 0);
    let sorted: BTreeMap<_, _> = entries.map(|e| (e.key, e)).collect();
    let mut body = Vec::with_capacity(sorted.len() * 18);
    let (mut pc, mut pb) = (0, 0);
    for e in sorted.values() {
        let mut r = Vec::from(e.key);
        r.extend(&e.location.to_be_bytes()[3..]);
        r.extend(e.size.to_le_bytes());
        (pc, pb) = lookup(&r, pc, pb);
        body.extend(r);
    }
    bytes.extend((body.len() as u32).to_le_bytes());
    bytes.extend(pc.to_le_bytes());
    bytes.extend(body);
    bytes.resize((bytes.len() + 0x7800 + 0xffff) & !0xffff, 0);
    bytes
}
fn entry_header(mut key: Key, size: u32, location: u64, full: bool, flags: u16) -> [u8; 30] {
    const TABLE: [u32; 16] = [
        0x049396b8, 0x72a82a9b, 0xee626cca, 0x9917754f, 0x15de40b1, 0xf5a8a9b6, 0x421eac7e,
        0xa9d55c9a, 0x317fd40c, 0x04faf80d, 0x3d6be971, 0x52933cfd, 0x27f64b7d, 0xc6f5c11b,
        0xd5757e3a, 0x6c388745,
    ];
    if !full {
        key[9..].fill(0);
    }
    key.reverse();
    let mut h = [0; 30];
    h[..16].copy_from_slice(&key);
    h[16..20].copy_from_slice(&(size + 30).to_le_bytes());
    h[20..22].copy_from_slice(&flags.to_le_bytes());
    let a = lookup(&h[..22], 0x3d6be971, 0).0;
    h[22..26].copy_from_slice(&a.to_le_bytes());
    let p = location as u32;
    let x = p.wrapping_add(30);
    let encoded = (TABLE[(x & 15) as usize] ^ x).to_le_bytes();
    let mut folded = [0; 4];
    for i in 0..26 {
        folded[i & 3] ^= h[i];
    }
    for j in 0..4 {
        let i = 26 + j;
        h[i] = folded[i & 3] ^ encoded[(p.wrapping_add(i as u32) & 3) as usize];
    }
    h
}
impl Storage {
    pub fn open(directory: PathBuf) -> Result<Self> {
        let mut versions = [0; 16];
        let mut archives = Vec::new();
        for item in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name.is_ascii()
                && name.len() == 14
                && name.ends_with(".idx")
                && name[..10].bytes().all(|b| b.is_ascii_hexdigit())
            {
                let b = usize::from_str_radix(&name[..2], 16).unwrap();
                let v = u32::from_str_radix(&name[2..10], 16).unwrap();
                if b < 16 {
                    versions[b] = versions[b].max(v);
                }
            } else if let Some(id) = name
                .strip_prefix("data.")
                .and_then(|s| s.parse::<u16>().ok())
            {
                archives.push(id);
            }
        }
        if versions.contains(&0) {
            return Err("Existing CASC storage is missing an index bucket.".into());
        }
        let mut entries = HashMap::new();
        for (b, v) in versions.iter().enumerate() {
            let path = directory.join(format!("{b:02x}{v:08x}.idx"));
            let data = fs::read(&path).map_err(|e| e.to_string())?;
            if data.get(10) != Some(&(b as u8)) {
                return Err("CASC index bucket mismatch.".into());
            }
            entries.extend(index(&data)?);
        }
        let first = *archives
            .iter()
            .min()
            .ok_or("No CASC data archives found.")?;
        let mut header = [0u8; 30];
        File::open(directory.join(format!("data.{first:03}")))
            .and_then(|mut file| file.read_exact(&mut header))
            .map_err(|e| e.to_string())?;
        if le(&header, 16)? != 30 || header[20..22] != [1, 0] {
            return Err("Unsupported CASC segment-header layout.".into());
        }
        let mut base_key: Key = header[..16].try_into().unwrap();
        base_key.reverse();
        let archive = archives
            .into_iter()
            .max()
            .ok_or("No existing CASC archives found.")?
            .checked_add(1)
            .ok_or("CASC archive number overflow.")?;
        Ok(Self {
            directory,
            entries,
            versions,
            base_key,
            archive,
            offset: 0,
            new_files: Vec::new(),
            index_committed: false,
        })
    }
    pub fn contains(&self, key: &Key) -> bool {
        self.entries.contains_key(&key[..9])
    }
    pub fn read(&self, key: &Key) -> Result<Vec<u8>> {
        let e = self
            .entries
            .get(&key[..9])
            .ok_or("Content not in local CASC storage.")?;
        if e.size < 30 || e.size > 1024 * 1024 * 1024 {
            return Err("Invalid local CASC file size.".into());
        }
        let archive = e.location >> 30;
        let offset = e.location & (LIMIT - 1);
        let mut file = File::open(self.directory.join(format!("data.{archive:03}")))
            .map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut header = [0; 30];
        file.read_exact(&mut header).map_err(|e| e.to_string())?;
        let mut original: Key = header[..16].try_into().unwrap();
        original.reverse();
        if original[..9] != key[..9] || le(&header, 16)? != e.size {
            return Err("CASC entry header does not match its index.".into());
        }
        let expected = entry_header(
            original,
            e.size - 30,
            e.location,
            true,
            u16::from_le_bytes(header[20..22].try_into().unwrap()),
        );
        if expected != header {
            return Err("CASC entry header checksum failed.".into());
        }
        let mut bytes = vec![0; e.size as usize - 30];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    pub fn verify(&self, key: &Key) -> Result<()> {
        let e = self.entries.get(&key[..9]).ok_or("Missing local content")?;
        if e.size < 38 || e.size > 1024 * 1024 * 1024 {
            return Err("Invalid CASC entry size".into());
        }
        let mut file = File::open(self.directory.join(format!("data.{:03}", e.location >> 30)))
            .map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(e.location & (LIMIT - 1)))
            .map_err(|e| e.to_string())?;
        let mut header = [0; 30];
        file.read_exact(&mut header).map_err(|e| e.to_string())?;
        let mut original: Key = header[..16].try_into().unwrap();
        original.reverse();
        if original[..9] != key[..9]
            || le(&header, 16)? != e.size
            || entry_header(
                original,
                e.size - 30,
                e.location,
                true,
                u16::from_le_bytes(header[20..22].try_into().unwrap()),
            ) != header
        {
            return Err("Invalid CASC entry header".into());
        }
        super::formats::verify_reader(file.take((e.size - 30) as u64), (e.size - 30) as u64, *key)
    }
    pub fn append_file(&mut self, key: Key, path: &Path, full: bool) -> Result<()> {
        if self.contains(&key) {
            return Ok(());
        }
        let mut input = File::open(path).map_err(|e| e.to_string())?;
        let size = u32::try_from(input.metadata().map_err(|e| e.to_string())?.len())
            .map_err(|_| "CASC blob exceeds archive limit.")?;
        if u64::from(size) + 510 > LIMIT {
            return Err("CASC blob exceeds archive limit.".into());
        }
        if self.offset == 0 || self.offset + u64::from(size) + 30 > LIMIT {
            if self.offset != 0 {
                self.archive = self
                    .archive
                    .checked_add(1)
                    .ok_or("CASC archive overflow.")?;
            }
            if self.archive >= 1023 {
                return Err("CASC storage has exhausted archive slots.".into());
            }
            let path = self.directory.join(format!("data.{:03}", self.archive));
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            self.new_files.push(path);
            for b in 0..16 {
                let mut k = self.base_key;
                k[1] = self.archive as u8;
                k[2] = (self.archive >> 8) as u8;
                for byte in 0..=255 {
                    k[0] = byte;
                    if (bucket(&k) + 1) & 15 == b {
                        break;
                    }
                }
                let location = (u64::from(self.archive) << 30) | (b * 30) as u64;
                file.write_all(&entry_header(k, 0, location, true, 1))
                    .map_err(|e| e.to_string())?;
                let entry = Entry {
                    bucket: b as u8,
                    key: k[..9].try_into().unwrap(),
                    location,
                    size: 30,
                };
                self.entries.insert(entry.key, entry);
            }
            self.offset = 480;
        }
        let path = self.directory.join(format!("data.{:03}", self.archive));
        let mut file = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let location = (u64::from(self.archive) << 30) | self.offset;
        file.write_all(&entry_header(key, size, location, full, 0))
            .map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut input, &mut file).map_err(|e| e.to_string())?;
        if copied != u64::from(size) {
            return Err("Cached game-file size changed during apply.".into());
        }
        self.offset += u64::from(size) + 30;
        let entry = Entry {
            bucket: bucket(&key) as u8,
            key: key[..9].try_into().unwrap(),
            location,
            size: size + 30,
        };
        self.entries.insert(entry.key, entry);
        Ok(())
    }
    pub fn commit(&mut self) -> Result<()> {
        for file in &self.new_files {
            File::open(file)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        let path = self.directory.join("shmem");
        let mut bytes = fs::read(&path).map_err(|e| format!("Cannot read CASC shmem: {e}"))?;
        let protocol = le(&bytes, 0)?;
        let table = match protocol {
            4 if le(&bytes, 4)? == 0x150 => 0x150,
            5 => ((le(&bytes, 0x10c)? as usize) + 0xfff) & !0xfff,
            _ => return Err("Unsupported CASC shared-memory layout.".into()),
        };
        if le(&bytes, 0x108)? != 0x2ab8
            || table
                .checked_add(0x2ab8)
                .is_none_or(|end| end > bytes.len())
        {
            return Err("Unsupported CASC free-space table.".into());
        }
        let mut versions = self.versions;
        // Complete new index generations are written before publishing shmem.
        self.index_committed = true;
        for (b, version) in versions.iter_mut().enumerate() {
            *version = version
                .checked_add(1)
                .ok_or("CASC index version overflow.")?;
            let bytes = serialized_index(
                b,
                self.entries
                    .values()
                    .filter(|e| e.bucket as usize == b)
                    .cloned(),
            );
            atomic_write(
                &self.directory.join(format!("{b:02x}{version:08x}.idx")),
                &bytes,
            )?;
        }
        // All published index generations retain every old entry.
        for (b, v) in versions.iter().enumerate() {
            bytes[0x110 + b * 4..0x114 + b * 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[table..table + 0x2ab8].fill(0);
        bytes[table..table + 4].copy_from_slice(&1u32.to_le_bytes());
        let mut spans = Vec::new();
        for archive in 0..1023 {
            let length = fs::metadata(self.directory.join(format!("data.{archive:03}")))
                .map(|m| m.len())
                .unwrap_or(0);
            if length > LIMIT {
                return Err("Invalid CASC archive size.".into());
            }
            if LIMIT - length > 64 {
                spans.push((LIMIT - length, (archive as u64) << 30 | length));
            }
        }
        bytes[table + 4..table + 8].copy_from_slice(&(spans.len() as u32).to_le_bytes());
        for (i, (size, offset)) in spans.iter().enumerate() {
            bytes[table + 0x20 + 5 * i..table + 0x25 + 5 * i]
                .copy_from_slice(&size.to_be_bytes()[3..]);
            bytes[table + 0x20 + 5 * 1090 + 5 * i..table + 0x25 + 5 * 1090 + 5 * i]
                .copy_from_slice(&offset.to_be_bytes()[3..]);
        }
        atomic_write(&path, &bytes)?;
        self.versions = versions;
        Ok(())
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        if !self.index_committed {
            for path in &self.new_files {
                let _ = fs::remove_file(path);
            }
        }
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    File::open(path.parent().ok_or("Missing target directory.")?)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn index_round_trip_preserves_locations_and_checksums() {
        let e = Entry {
            bucket: bucket(&[9; 9]) as u8,
            key: [9; 9],
            location: (7 << 30) | 480,
            size: 1234,
        };
        let mut bytes = serialized_index(bucket(&e.key), [e.clone()].into_iter());
        let parsed = index(&bytes).unwrap();
        assert_eq!(parsed[&e.key].location, e.location);
        bytes[45] ^= 1;
        assert!(index(&bytes).is_err());
    }
    #[test]
    fn header_checksums_depend_on_position() {
        let a = entry_header([1; 16], 100, 480, false, 0);
        let b = entry_header([1; 16], 100, 580, false, 0);
        assert_eq!(&a[..26], &b[..26]);
        assert_ne!(&a[26..], &b[26..]);
        assert_eq!(le(&a, 16).unwrap(), 130);
    }
}
/// Create only missing bootstrap files for a newly claimed installation.
pub fn initialize(directory: &Path, windows_path: &str, base: Key) -> Result<()> {
    if windows_path.len() + 1 > 256 {
        return Err("The installation path is too long for CASC storage.".into());
    }
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let archive = directory.join("data.000");
    let mut bytes = Vec::new();
    let mut entries = Vec::new();
    for b in 0..16 {
        let mut key = base;
        key[1] = 0;
        key[2] = 0;
        for byte in 0..=255 {
            key[0] = byte;
            if (bucket(&key) + 1) & 15 == b {
                break;
            }
        }
        bytes.extend(entry_header(key, 0, (b * 30) as u64, true, 1));
        entries.push(Entry {
            bucket: b as u8,
            key: key[..9].try_into().unwrap(),
            location: (b * 30) as u64,
            size: 30,
        });
    }
    if !archive.exists() {
        atomic_write(&archive, &bytes)?;
    }
    for b in 0..16 {
        let path = directory.join(format!("{b:02x}00000001.idx"));
        if !path.exists() {
            atomic_write(
                &path,
                &serialized_index(
                    b,
                    entries.iter().filter(|e| e.bucket as usize == b).cloned(),
                ),
            )?;
        }
    }
    let path = directory.join("shmem");
    if !path.exists() {
        let mut bytes = vec![0; 0x2c10];
        bytes[..4].copy_from_slice(&4u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x150u32.to_le_bytes());
        bytes[8..8 + windows_path.len()].copy_from_slice(windows_path.as_bytes());
        bytes[0x108..0x10c].copy_from_slice(&0x2ab8u32.to_le_bytes());
        bytes[0x10c..0x110].copy_from_slice(&0x150u32.to_le_bytes());
        for i in 0..16 {
            bytes[0x110 + i * 4..0x114 + i * 4].copy_from_slice(&1u32.to_le_bytes());
        }
        bytes[0x150..0x154].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x154..0x158].copy_from_slice(&1023u32.to_le_bytes());
        for i in 0..1023 {
            let length = if i == 0 { 480 } else { 0 };
            let size = LIMIT - length;
            let offset = (i as u64) << 30 | length;
            bytes[0x170 + 5 * i..0x175 + 5 * i].copy_from_slice(&size.to_be_bytes()[3..]);
            bytes[0x170 + 5 * 1090 + 5 * i..0x175 + 5 * 1090 + 5 * i]
                .copy_from_slice(&offset.to_be_bytes()[3..]);
        }
        atomic_write(&path, &bytes)?;
    }
    Ok(())
}
