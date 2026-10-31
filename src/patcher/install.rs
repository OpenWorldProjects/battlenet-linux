use super::{Result, cancelled, formats, http, storage, text};
use crate::{catalog, runner::Game};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::PathBuf, sync::atomic::AtomicBool};
#[derive(Serialize, Deserialize)]
struct Claim {
    product: String,
    base: formats::Key,
    #[serde(default)]
    info: String,
}
fn read_config(game: &Game, cancel: &AtomicBool) -> Result<(Vec<String>, Vec<String>)> {
    cancelled(cancel)?;
    let product = catalog::product(&game.product_code)?;
    let client = http()?;
    let versions = formats::table(&text(
        &client,
        &format!("https://eu.version.battle.net/{}/versions", product.slug),
    )?)?;
    let version = super::row(&versions, "Region", &game.region)?;
    let hash = super::field(version, "ProductConfig")?;
    let expected = formats::key(hash)?;
    let cdns = formats::table(&text(
        &client,
        &format!("https://eu.version.battle.net/{}/cdns", product.slug),
    )?)?;
    let cdn = super::row(&cdns, "Name", &game.region)?;
    let path = cdn
        .get("ConfigPath")
        .map(String::as_str)
        .unwrap_or("tpr/configs/data");
    if path.contains("..")
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-".contains(&b))
    {
        return Err("Invalid product-config path.".into());
    }
    let bytes = client
        .get(format!(
            "https://eu.cdn.blizzard.com/{path}/{}/{}/{hash}",
            &hash[..2],
            &hash[2..4]
        ))
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 || formats::hash(&bytes) != expected {
        return Err("Product-configuration checksum mismatch.".into());
    }
    let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let all = &json["all"]["config"];
    let win = &json["platform"]["win"]["config"];
    if all["data_dir"].as_str().map(|s| s.trim_end_matches('/')) != Some(product.data_dir) {
        return Err(
            "Blizzard changed this game's data layout; refusing an unverified installation.".into(),
        );
    }
    let list = |value: &serde_json::Value| {
        value
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let mut tags = list(&win["tags"]);
    if tags.is_empty() && !product.containerless {
        return Err("Product configuration has no Windows installation tags.".into());
    }
    tags.extend(list(&win["tags_64bit"]));
    tags.extend(list(&all["extra_tags"]));
    tags.extend(list(&all["noigr_tags"]));
    let locales = list(&all["supported_locales"]);
    Ok((tags, locales))
}
pub(super) fn prepare(game: &Game, cancel: &AtomicBool) -> Result<PathBuf> {
    let product = catalog::product(&game.product_code)?;
    let root = catalog::target_root(game)?;
    if root.join(".build.info").exists() && !root.join(".bnet-linux-install.json").exists() {
        if !root.join(product.data_dir).is_dir() && !root.join(product.executable).is_file() {
            return Err(format!(
                "This folder does not contain {}. Choose its own installation directory.",
                product.name
            ));
        }
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if product.containerless {
            let data = root.join(".bnet-linux-container");
            for name in ["data", "config", "indices"] {
                fs::create_dir_all(data.join(name)).map_err(|e| e.to_string())?;
            }
            let mut base = [0; 16];
            fs::File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut base))
                .map_err(|e| e.to_string())?;
            let windows = format!(
                "Z:{}\\.bnet-linux-container\\data\\index",
                root.to_string_lossy().replace('/', "\\")
            );
            storage::initialize(&data.join("data"), &windows, base)?;
        }
        return Ok(root);
    }
    let marker = root.join(".bnet-linux-install.json");
    if !marker.exists() {
        if root.exists() {
            for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_name() != ".bnet-linux-update.lock" {
                    return Err("Choose an empty directory for a new game installation.".into());
                }
            }
        }
        let (tags, locales) = read_config(game, cancel)?;
        if !locales.is_empty() && !locales.iter().any(|s| s.eq_ignore_ascii_case(&game.locale)) {
            return Err("This game does not publish the selected language.".into());
        }
        cancelled(cancel)?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut base = [0; 16];
        fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut base))
            .map_err(|e| e.to_string())?;
        let common = format!("{} {}?", tags.join(" "), game.region);
        let tags = format!(
            "{common} {} speech?:{common} {} text?",
            game.locale, game.locale
        );
        let info = format!(
            "Branch!STRING:0|Active!DEC:1|Build Key!HEX:16|CDN Key!HEX:16|Install Key!HEX:16|IM Size!DEC:4|CDN Path!STRING:0|CDN Hosts!STRING:0|CDN Servers!STRING:0|Tags!STRING:0|Armadillo!STRING:0|Last Activated!STRING:0|Version!STRING:0|KeyRing!HEX:16|Product!STRING:0\n{}|1|{}|{}||||||{}|||0.0.0.0||{}\n",
            game.region.to_lowercase(),
            "0".repeat(32),
            "0".repeat(32),
            tags,
            product.slug
        );
        storage::atomic_write(
            &marker,
            &serde_json::to_vec(&Claim {
                product: product.code.into(),
                base,
                info: info.clone(),
            })
            .map_err(|e| e.to_string())?,
        )?;
        storage::atomic_write(&root.join(".build.info"), info.as_bytes())?;
    }
    let claim: Claim = serde_json::from_slice(&fs::read(&marker).map_err(|e| e.to_string())?)
        .map_err(|_| "Invalid installation ownership marker.")?;
    if claim.product != product.code {
        return Err("This directory belongs to a different game installation.".into());
    }
    if !root.join(".build.info").exists() {
        if claim.info.is_empty() {
            return Err("Missing installation bootstrap metadata.".into());
        }
        storage::atomic_write(&root.join(".build.info"), claim.info.as_bytes())?;
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let data = root.join(if product.containerless {
        ".bnet-linux-container"
    } else {
        product.data_dir
    });
    for name in ["data", "config", "indices"] {
        fs::create_dir_all(data.join(name)).map_err(|e| e.to_string())?;
    }
    let windows = format!(
        "Z:{}\\{}\\data\\index",
        root.to_string_lossy().replace('/', "\\"),
        if product.containerless {
            ".bnet-linux-container"
        } else {
            product.data_dir
        }
    );
    storage::initialize(&data.join("data"), &windows, claim.base)?;
    Ok(root)
}
