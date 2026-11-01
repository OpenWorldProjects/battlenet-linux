//! Private, account/region-bound credentials issued by Battle.net itself.
use crate::{auth::Account, runner};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, Zeroizing};
const MAX_TOKEN: usize = 16 * 1024;
const MAX_CACHE: u64 = 128 * 1024;
#[derive(Serialize, Deserialize)]
struct Saved {
    account: String,
    region: String,
    renewed: u64,
    credential: Vec<u8>,
}
impl Drop for Saved {
    fn drop(&mut self) {
        self.credential.zeroize();
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn path(region: &str, account: &Account) -> Result<PathBuf, String> {
    account.validate()?;
    if !matches!(region, "EU" | "US" | "KR") {
        return Err("Unsupported session region.".into());
    }
    Ok(runner::session_dir()?.join(format!(
        "chat-{:x}.json",
        md5::compute(format!("{region}|{}", account.account_id))
    )))
}
pub fn load(region: &str, account: &Account) -> Option<Zeroizing<Vec<u8>>> {
    let file = File::open(path(region, account).ok()?).ok()?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAX_CACHE + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_CACHE {
        return None;
    }
    decode(&bytes, region, account, now())
}
fn decode(bytes: &[u8], region: &str, account: &Account, time: u64) -> Option<Zeroizing<Vec<u8>>> {
    if bytes.len() as u64 > MAX_CACHE {
        return None;
    }
    let saved: Saved = serde_json::from_slice(bytes).ok()?;
    if saved.account != account.account_id
        || saved.region != region
        || saved.credential.is_empty()
        || saved.credential.len() > MAX_TOKEN
        || saved.renewed > time
        || time - saved.renewed > 30 * 86400
    {
        return None;
    }
    Some(Zeroizing::new(saved.credential.clone()))
}
pub fn save(region: &str, account: &Account, credential: &[u8]) -> Result<(), String> {
    if credential.is_empty() || credential.len() > MAX_TOKEN {
        return Err("Invalid renewed credential size.".into());
    }
    let path = path(region, account)?;
    let saved = Saved {
        account: account.account_id.clone(),
        region: region.into(),
        renewed: now(),
        credential: credential.to_vec(),
    };
    let bytes = Zeroizing::new(
        serde_json::to_vec(&saved).map_err(|_| "Could not encode renewed session.")?,
    );
    write_atomic(&path, &bytes)
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| {
        let mut file = File::options()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&staging)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&staging, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result.map_err(|_| "Could not save the renewed Battle.net session.".into())
}
pub fn clear(region: &str, account: &Account) {
    if let Ok(path) = path(region, account) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn account(id: &str) -> Account {
        Account {
            account_id: id.into(),
            battle_tag: None,
        }
    }
    fn fixture(credential: Vec<u8>) -> Vec<u8> {
        serde_json::to_vec(&Saved {
            account: "12345".into(),
            region: "EU".into(),
            renewed: 1000,
            credential,
        })
        .unwrap()
    }
    #[test]
    fn cached_credentials_are_bound_to_account_region_and_retention() {
        let bytes = fixture(b"synthetic-credential".to_vec());
        assert!(decode(&bytes, "EU", &account("12345"), 1001).is_some());
        assert!(decode(&bytes, "US", &account("12345"), 1001).is_none());
        assert!(decode(&bytes, "EU", &account("98765"), 1001).is_none());
        assert!(decode(&bytes, "EU", &account("12345"), 999).is_none());
        assert!(decode(&bytes, "EU", &account("12345"), 1000 + 31 * 86400).is_none());
    }
    #[test]
    fn corrupt_empty_and_oversized_credentials_are_not_replayed() {
        assert!(decode(b"invalid", "EU", &account("12345"), 1001).is_none());
        assert!(decode(&fixture(vec![]), "EU", &account("12345"), 1001).is_none());
        assert!(
            decode(
                &fixture(vec![1; MAX_TOKEN + 1]),
                "EU",
                &account("12345"),
                1001
            )
            .is_none()
        );
        assert!(
            decode(
                &vec![1; MAX_CACHE as usize + 1],
                "EU",
                &account("12345"),
                1001
            )
            .is_none()
        );
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    #[test]
    fn stored_credentials_remain_private_during_atomic_replacement() {
        let dir = std::env::temp_dir().join(format!(
            "bnet-private-session-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("credential.json");
        write_atomic(&path, b"synthetic-one").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        write_atomic(&path, b"synthetic-two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"synthetic-two");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
