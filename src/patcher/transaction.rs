//! A durable loose-file journal. The build marker is the final commit point.
use super::{
    Plan, destination,
    formats::{self, Key, Result},
    hash_file, relative,
    storage::atomic_write,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
struct Change {
    path: String,
    existed: bool,
    old_hash: Option<Key>,
    new_hash: Key,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    staging: String,
    old_marker: Vec<u8>,
    new_marker: Key,
    files: Vec<Change>,
}
fn location(root: &Path) -> Result<PathBuf> {
    destination(root, Path::new(".bnet-linux-update/transaction.json"))
}
pub(super) fn begin(plan: &Plan, staging: &Path) -> Result<()> {
    let path = location(&plan.root)?;
    if path.exists() {
        return Err(
            "An interrupted update must be recovered before applying another patch.".into(),
        );
    }
    let mut files = Vec::new();
    for file in &plan.loose {
        let target = destination(&plan.root, &file.relative)?;
        let existed = target.exists();
        files.push(Change {
            path: file.file.path.clone(),
            existed,
            old_hash: if existed {
                Some(hash_file(&target)?)
            } else {
                None
            },
            new_hash: file.file.content,
        });
    }
    let journal = Journal {
        staging: staging
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid staging directory.")?
            .into(),
        old_marker: plan.original.clone(),
        new_marker: formats::hash(&plan.updated),
        files,
    };
    atomic_write(
        &path,
        &serde_json::to_vec(&journal).map_err(|e| e.to_string())?,
    )
}
pub(super) fn finish(root: &Path) -> Result<()> {
    let path = location(root)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
        fs::File::open(path.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub(super) fn recover(root: &Path) -> Result<bool> {
    let path = location(root)?;
    if !path.exists() {
        return Ok(false);
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Update recovery journal exceeds supported size.".into());
    }
    let journal: Journal = serde_json::from_slice(&bytes)
        .map_err(|_| "Cannot read the interrupted-update journal.")?;
    if journal.staging.is_empty()
        || journal.staging.len() > 100
        || !journal
            .staging
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-')
        || journal.old_marker.len() > 1024 * 1024
        || journal.files.len() > 100_000
    {
        return Err("Invalid interrupted-update journal.".into());
    }
    let staging = destination(
        root,
        &Path::new(".bnet-linux-update").join(&journal.staging),
    )?;
    let marker = fs::read(root.join(".build.info")).map_err(|e| e.to_string())?;
    if formats::hash(&marker) != journal.new_marker {
        if marker != journal.old_marker {
            return Err("Another updater changed the active build after an interrupted patch. Recovery files were preserved.".into());
        }
        for file in journal.files.iter().rev() {
            let relative = relative(&file.path)?;
            let target = destination(root, &relative)?;
            let backup = destination(&staging.join("backup"), &relative)?;
            if file.existed {
                if backup.is_file() {
                    if Some(hash_file(&backup)?) != file.old_hash {
                        return Err("The original recovery file failed its checksum.".into());
                    }
                    if target.exists() {
                        if hash_file(&target)? != file.new_hash {
                            return Err(
                                "A patched file was changed externally; its backup was preserved."
                                    .into(),
                            );
                        }
                        fs::remove_file(&target).map_err(|e| e.to_string())?;
                    }
                    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                    fs::rename(backup, &target).map_err(|e| e.to_string())?;
                    super::sync_directories(target.parent().unwrap(), root)?;
                } else if !target.is_file() || Some(hash_file(&target)?) != file.old_hash {
                    return Err(
                        "An interrupted patch needs its preserved original files for recovery."
                            .into(),
                    );
                }
            } else if target.exists() {
                if hash_file(&target)? != file.new_hash {
                    return Err("A new patch file changed externally; refusing to remove it during recovery.".into());
                }
                fs::remove_file(&target).map_err(|e| e.to_string())?;
            }
        }
    }
    finish(root)?;
    if staging.is_dir() {
        fs::remove_dir_all(staging).map_err(|e| e.to_string())?;
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("bnet-recovery-{}-{name}", std::process::id()));
        fs::create_dir_all(path.join(".bnet-linux-update/abc-123/backup/Support64")).unwrap();
        path
    }
    #[test]
    fn restores_files_after_interrupted_commit() {
        let root = fixture("restore");
        fs::create_dir_all(root.join("Support64")).unwrap();
        fs::write(root.join(".build.info"), b"old").unwrap();
        fs::write(root.join("Support64/game.exe"), b"new").unwrap();
        fs::write(
            root.join(".bnet-linux-update/abc-123/backup/Support64/game.exe"),
            b"old exe",
        )
        .unwrap();
        let journal = Journal {
            staging: "abc-123".into(),
            old_marker: b"old".to_vec(),
            new_marker: formats::hash(b"new marker"),
            files: vec![Change {
                path: "Support64/game.exe".into(),
                existed: true,
                old_hash: Some(formats::hash(b"old exe")),
                new_hash: formats::hash(b"new"),
            }],
        };
        atomic_write(
            &location(&root).unwrap(),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert!(recover(&root).unwrap());
        assert_eq!(
            fs::read(root.join("Support64/game.exe")).unwrap(),
            b"old exe"
        );
        assert_eq!(fs::read(root.join(".build.info")).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn keeps_committed_files_when_marker_was_published() {
        let root = fixture("commit");
        fs::write(root.join(".build.info"), b"new marker").unwrap();
        let journal = Journal {
            staging: "abc-123".into(),
            old_marker: b"old".to_vec(),
            new_marker: formats::hash(b"new marker"),
            files: vec![],
        };
        atomic_write(
            &location(&root).unwrap(),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert!(recover(&root).unwrap());
        assert_eq!(fs::read(root.join(".build.info")).unwrap(), b"new marker");
        fs::remove_dir_all(root).unwrap();
    }
}
