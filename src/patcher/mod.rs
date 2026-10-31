mod formats;
mod install;
mod storage;
mod transaction;
use crate::{
    runner::{self, Game},
    updates,
};
use formats::{Key, Location, Result};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use storage::{Storage, atomic_write};

#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub message: String,
    pub completed: u64,
    pub total: u64,
}
#[derive(Debug)]
pub struct Summary {
    pub version: String,
    pub storage_files: usize,
    pub loose_files: usize,
    pub download_bytes: u64,
}
fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("Update cancelled; the installed build is unchanged.".into())
    } else {
        Ok(())
    }
}
fn message(progress: &impl Fn(Progress), text: impl Into<String>) {
    progress(Progress {
        message: text.into(),
        ..Progress::default()
    });
}
fn hash_file(path: &Path) -> Result<Key> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut context = md5::Context::new();
    let mut bytes = [0; 65536];
    loop {
        let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        context.consume(&bytes[..n]);
    }
    Ok(context.finalize().0)
}
fn relative(path: &str) -> Result<PathBuf> {
    if path.is_empty()
        || path.contains(['\\', ':', '\0'])
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return Err("Unsafe path in Blizzard install manifest.".into());
    }
    let path = PathBuf::from(path);
    let first = path
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("");
    if first.starts_with(".bnet-linux-")
        || first.starts_with(".bnet-linux-update")
        || first == ".build.info"
        || first == ".product.db"
    {
        return Err("Install manifest tries to replace protected installation metadata.".into());
    }
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Unsafe manifest path.".into());
    }
    Ok(path)
}
fn destination(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("An update target is a symbolic link; refusing to replace files outside the installation.".into());
        }
    }
    Ok(path)
}
fn http() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())
}
fn text(client: &reqwest::blocking::Client, url: &str) -> Result<String> {
    let response = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Blizzard download failed: {e}"))?;
    let mut text = String::new();
    response
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 8 * 1024 * 1024 {
        return Err("Blizzard configuration is too large.".into());
    }
    Ok(text)
}
#[derive(Clone)]
struct Cdn {
    client: reqwest::blocking::Client,
    bases: Vec<String>,
    cache: PathBuf,
}
impl Cdn {
    fn request(
        &self,
        kind: &str,
        key: Key,
        suffix: &str,
        location: Option<&Location>,
    ) -> Result<reqwest::blocking::Response> {
        let hash = location
            .map(|l| l.archive.clone())
            .unwrap_or_else(|| formats::hex(&key));
        formats::key(&hash)?;
        let mut last = "No CDN hosts available.".to_owned();
        for base in &self.bases {
            let url = format!(
                "{base}/{kind}/{}/{}/{}{suffix}",
                &hash[..2],
                &hash[2..4],
                hash
            );
            let mut request = self.client.get(url);
            if let Some(location) = location {
                let end = location
                    .offset
                    .checked_add(location.size)
                    .and_then(|n| n.checked_sub(1))
                    .ok_or("Invalid CDN range.")?;
                request = request.header(
                    reqwest::header::RANGE,
                    format!("bytes={}-{}", location.offset, end),
                );
            }
            match request
                .send()
                .and_then(|response| response.error_for_status())
            {
                Ok(response) => {
                    if location.is_some()
                        && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
                    {
                        last = "CDN ignored the requested archive range.".into();
                        continue;
                    }
                    if let Some(location) = location {
                        let expected = format!(
                            "bytes {}-{}",
                            location.offset,
                            location.offset + location.size - 1
                        );
                        if !response
                            .headers()
                            .get(reqwest::header::CONTENT_RANGE)
                            .and_then(|s| s.to_str().ok())
                            .is_some_and(|s| s.starts_with(&(expected + "/")))
                        {
                            last = "CDN returned an unexpected archive range.".into();
                            continue;
                        }
                    }
                    return Ok(response);
                }
                Err(error) => last = error.to_string(),
            }
        }
        Err(format!(
            "Could not download verified Blizzard content: {last}"
        ))
    }
    fn config(&self, key: Key) -> Result<Vec<u8>> {
        let path = self.cache.join(format!("{}.config", formats::hex(&key)));
        if let Ok(bytes) = fs::read(&path) {
            if formats::hash(&bytes) == key {
                return Ok(bytes);
            }
        }
        let mut bytes = Vec::new();
        self.request("config", key, "", None)?
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 8 * 1024 * 1024 || formats::hash(&bytes) != key {
            return Err("Blizzard build-configuration checksum mismatch.".into());
        }
        atomic_write(&path, &bytes)?;
        Ok(bytes)
    }
    fn index(&self, key: Key, local: &Path) -> Result<Vec<u8>> {
        let name = format!("{}.index", formats::hex(&key));
        let cached = self.cache.join(&name);
        if let Ok(bytes) = fs::read(local.join(&name)) {
            return Ok(bytes);
        }
        if let Ok(bytes) = fs::read(&cached) {
            return Ok(bytes);
        }
        let mut bytes = Vec::new();
        self.request("data", key, ".index", None)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Blizzard archive index exceeds supported size.".into());
        }
        atomic_write(&cached, &bytes)?;
        Ok(bytes)
    }
    fn file(&self, item: &Item, cancel: &AtomicBool) -> Result<PathBuf> {
        cancelled(cancel)?;
        let path = self
            .cache
            .join(format!("{}.encoded", formats::hex(&item.key)));
        let verify = |path: &Path| -> Result<()> {
            if item.raw {
                if hash_file(path)? != item.key {
                    return Err("Patch-manifest checksum mismatch.".into());
                }
                Ok(())
            } else {
                formats::verify_file(path, item.key)
            }
        };
        if path.is_file() && verify(&path).is_ok() {
            return Ok(path);
        }
        let part = path.with_extension(format!("{}.part", std::process::id()));
        let mut response = self.request(
            if item.raw { "patch" } else { "data" },
            item.key,
            "",
            item.location.as_ref(),
        )?;
        let mut output = File::create(&part).map_err(|e| e.to_string())?;
        let mut buffer = [0; 65536];
        let mut length = 0u64;
        loop {
            cancelled(cancel)?;
            let n = response
                .read(&mut buffer)
                .map_err(|e| format!("Game download interrupted: {e}"))?;
            if n == 0 {
                break;
            }
            length += n as u64;
            if length > (1 << 30) - 510 {
                return Err("Downloaded content exceeds CASC archive limit.".into());
            }
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        let expected = item.location.as_ref().map(|l| l.size).unwrap_or(item.size);
        if expected != 0 && length != expected {
            return Err("Downloaded game-file size does not match its manifest.".into());
        }
        verify(&part)?;
        fs::rename(part, &path).map_err(|e| e.to_string())?;
        Ok(path)
    }
    fn decoded(
        &self,
        key: Key,
        content: Option<Key>,
        storage: &Storage,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>> {
        let blob = if storage.contains(&key) {
            storage.read(&key)?
        } else {
            let path = self.file(
                &Item {
                    key,
                    size: 0,
                    full: true,
                    raw: false,
                    location: None,
                },
                cancel,
            )?;
            let length = fs::metadata(&path).map_err(|e| e.to_string())?.len();
            if length > 256 * 1024 * 1024 {
                return Err("Blizzard metadata exceeds supported size.".into());
            }
            fs::read(path).map_err(|e| e.to_string())?
        };
        let bytes = formats::blte(&blob, key)?;
        if content.is_some_and(|key| formats::hash(&bytes) != key) {
            return Err("Decoded manifest checksum failed.".into());
        }
        Ok(bytes)
    }
}
#[derive(Clone)]
struct Item {
    key: Key,
    size: u64,
    full: bool,
    raw: bool,
    location: Option<Location>,
}
struct Loose {
    file: formats::InstallFile,
    relative: PathBuf,
    key: Key,
}
struct Plan {
    root: PathBuf,
    data: PathBuf,
    original: Vec<u8>,
    updated: Vec<u8>,
    version: String,
    build: Key,
    cdn_key: Key,
    build_config: Vec<u8>,
    cdn_config: Vec<u8>,
    storage: Storage,
    storage_keys: HashSet<Key>,
    items: Vec<Item>,
    loose: Vec<Loose>,
    cdn: Cdn,
}
fn row<'a>(
    rows: &'a [HashMap<String, String>],
    column: &str,
    region: &str,
) -> Result<&'a HashMap<String, String>> {
    rows.iter()
        .find(|row| {
            row.get(column)
                .is_some_and(|s| s.eq_ignore_ascii_case(region))
        })
        .ok_or_else(|| "Blizzard has no version for the selected region.".into())
}
fn field<'a>(row: &'a HashMap<String, String>, name: &str) -> Result<&'a str> {
    row.get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("Missing Blizzard field: {name}"))
}
fn pair(config: &HashMap<String, Vec<String>>, name: &str) -> Result<(Key, Key)> {
    let keys = config
        .get(name)
        .ok_or_else(|| format!("Build has no {name} manifest."))?;
    if keys.len() != 2 {
        return Err(format!("Unsupported {name} manifest configuration."));
    }
    Ok((formats::key(&keys[0])?, formats::key(&keys[1])?))
}
fn plan(
    game: &Game,
    cancel: &AtomicBool,
    progress: &impl Fn(Progress),
    repair: bool,
) -> Result<Plan> {
    let product = crate::catalog::product(&game.product_code)?;
    let root = crate::catalog::target_root(game)?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let data = root.join(if product.containerless {
        ".bnet-linux-container"
    } else {
        product.data_dir
    });
    if data.canonicalize().map_err(|e| e.to_string())? != data {
        return Err("Native patching requires CASC data inside the game installation.".into());
    }
    message(progress, "Reading the installed CASC indexes…");
    let mut storage = Storage::open(data.join("data"))?;
    let original = fs::read(root.join(".build.info")).map_err(|e| e.to_string())?;
    let installed =
        std::str::from_utf8(&original).map_err(|_| "Invalid installed build metadata.")?;
    let local_rows = formats::table(installed)?;
    let local_rows: Vec<_> = local_rows
        .into_iter()
        .filter(|r| {
            r.get("Product")
                .is_none_or(|p| p.is_empty() || p.eq_ignore_ascii_case(product.slug))
        })
        .collect();
    let current = local_rows
        .iter()
        .find(|r| {
            r.get("Active").map(String::as_str) == Some("1")
                && r.get("Branch")
                    .is_some_and(|s| s.eq_ignore_ascii_case(&game.region))
        })
        .or_else(|| {
            if game.product_code != "S2" {
                local_rows
                    .iter()
                    .find(|r| r.get("Active").map(String::as_str) == Some("1"))
            } else {
                None
            }
        })
        .ok_or("No active installation for the selected region.")?;
    let client = http()?;
    cancelled(cancel)?;
    message(progress, "Checking Blizzard's current build…");
    let versions = text(
        &client,
        &format!("https://eu.version.battle.net/{}/versions", product.slug),
    )?;
    let rows = formats::table(&versions)?;
    let target = row(&rows, "Region", &game.region)?;
    let version = updates::manifest_version(&versions, &game.region)?.name;
    let build = formats::key(field(target, "BuildConfig")?)?;
    let cdn_key = formats::key(field(target, "CDNConfig")?)?;
    let cdns = text(
        &client,
        &format!("https://eu.version.battle.net/{}/cdns", product.slug),
    )?;
    let rows = formats::table(&cdns)?;
    let cdn_row = row(&rows, "Name", &game.region)?;
    let path = field(cdn_row, "Path")?;
    if path.contains("..")
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'_' || b == b'-')
    {
        return Err("Invalid Blizzard CDN path.".into());
    }
    let mut bases = Vec::new();
    if let Some(servers) = cdn_row.get("Servers") {
        for server in servers.split_whitespace() {
            if let Ok(mut url) = url::Url::parse(server) {
                url.set_query(None);
                if url.scheme() == "https"
                    && url
                        .host_str()
                        .is_some_and(|h| h.ends_with(".blizzard.com") || h.ends_with(".battle.net"))
                {
                    bases.push(format!("{}/{path}", url.as_str().trim_end_matches('/')));
                }
            }
        }
    }
    for host in field(cdn_row, "Hosts")?.split_whitespace() {
        if host.ends_with(".blizzard.com") || host.ends_with(".battle.net") {
            let host = if host == "level3.blizzard.com" {
                "level3.ssl.blizzard.com"
            } else {
                host
            };
            bases.push(format!("https://{host}/{path}"));
        }
    }
    bases.sort();
    bases.dedup();
    if bases.is_empty() {
        return Err("Blizzard did not provide a secure CDN endpoint.".into());
    }
    let cache = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")
        .ok_or("Cannot locate patch cache.")?
        .cache_dir()
        .join("patch-content");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let cdn = Cdn {
        client,
        bases,
        cache,
    };
    let build_config = cdn.config(build)?;
    let cdn_config = cdn.config(cdn_key)?;
    let config = formats::config(&build_config)?;
    let archive_config = formats::config(&cdn_config)?;
    let tags = current
        .get("Tags")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| format!("Windows {}? {}", game.region, game.locale));
    message(progress, format!("Planning {} {version}…", product.name));
    if repair {
        for name in ["install", "download", "encoding"] {
            cancelled(cancel)?;
            let (_, key) = pair(&config, name)?;
            if storage.contains(&key) && storage.verify(&key).is_err() {
                storage.entries.remove(&key[..9]);
            }
        }
    }
    let (install_content, install_key) = pair(&config, "install")?;
    let (download_content, download_key) = pair(&config, "download")?;
    let (encoding_content, encoding_key) = pair(&config, "encoding")?;
    let loose_manifest = cdn.decoded(install_key, Some(install_content), &storage, cancel)?;
    let files = formats::install(&loose_manifest, &tags)?;
    let root_content = config
        .get("root")
        .and_then(|v| v.first())
        .map(|s| formats::key(s))
        .transpose()?;
    let mut wanted: HashSet<Key> = files.iter().map(|f| f.content).collect();
    if let Some(key) = root_content {
        wanted.insert(key);
    }
    let encoded = formats::encoding(
        &cdn.decoded(encoding_key, Some(encoding_content), &storage, cancel)?,
        &wanted,
    )?;
    let mut loose = Vec::new();
    let mut loose_keys = HashSet::new();
    for file in files {
        let mut relative = relative(&file.path)?;
        if !product.containerless
            && relative.components().next().is_some_and(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(product.data_dir.split('/').next().unwrap_or("Data"))
            })
        {
            return Err("Install manifest attempts to overwrite CASC storage.".into());
        }
        if !product.game_dir.is_empty() && !relative.starts_with(product.game_dir) {
            relative = PathBuf::from(product.game_dir).join(relative);
        }
        let key = *encoded
            .get(&file.content)
            .ok_or("File is missing from Blizzard's encoding manifest.")?;
        loose_keys.insert(key);
        let destination = destination(&root, &relative)?;
        if fs::metadata(&destination).is_ok_and(|m| m.is_file() && m.len() == file.size)
            && hash_file(&destination).ok() == Some(file.content)
        {
            continue;
        }
        loose.push(Loose {
            file,
            relative,
            key,
        });
    }
    let mut items: HashMap<Key, Item> = HashMap::new();
    for (name, values) in &config {
        if name == "size" || values.len() != 2 {
            continue;
        }
        if let (Ok(_), Ok(key)) = (formats::key(&values[0]), formats::key(&values[1])) {
            let size = config
                .get(&format!("{name}-size"))
                .and_then(|v| v.last())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if !storage.contains(&key) {
                items.insert(
                    key,
                    Item {
                        key,
                        size,
                        full: true,
                        raw: false,
                        location: None,
                    },
                );
            }
        }
    }
    if let Some(keys) = config.get("patch") {
        if keys.len() == 1 {
            let key = formats::key(&keys[0])?;
            if !storage.contains(&key) {
                let size = config
                    .get("patch-size")
                    .and_then(|v| v.first())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                items.insert(
                    key,
                    Item {
                        key,
                        size,
                        full: true,
                        raw: true,
                        location: None,
                    },
                );
            }
        }
    }
    if let Some(content) = root_content {
        let key = *encoded
            .get(&content)
            .ok_or("Missing encoded root manifest.")?;
        if repair && storage.contains(&key) && storage.verify(&key).is_err() {
            storage.entries.remove(&key[..9]);
        }
        if !storage.contains(&key) {
            items.entry(key).or_insert(Item {
                key,
                size: 0,
                full: false,
                raw: false,
                location: None,
            });
        }
    }
    let manifest = cdn.decoded(download_key, Some(download_content), &storage, cancel)?;
    let downloads = formats::download(&manifest, &tags)?;
    let count = downloads.len();
    for (i, (key, size)) in downloads.into_iter().enumerate() {
        if repair && !product.containerless && storage.contains(&key) {
            cancelled(cancel)?;
            progress(Progress {
                message: format!("Scanning game data ({}/{count})…", i + 1),
                completed: i as u64,
                total: count as u64,
            });
            if storage.verify(&key).is_err() {
                cancelled(cancel)?;
                storage.entries.remove(&key[..9]);
            }
        }
        if !storage.contains(&key) && !loose_keys.contains(&key) {
            items.entry(key).or_insert(Item {
                key,
                size,
                full: false,
                raw: false,
                location: None,
            });
        }
    }
    if product.containerless {
        items.clear();
    }
    let mut needed: HashSet<_> = items
        .keys()
        .copied()
        .chain(loose.iter().map(|f| f.key))
        .collect();
    let mut locations = HashMap::new();
    if !needed.is_empty() {
        let archives = archive_config
            .get("archives")
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for (i, archive) in archives.iter().enumerate() {
            cancelled(cancel)?;
            message(
                progress,
                format!("Locating patch content ({}/{})…", i + 1, archives.len()),
            );
            let key = formats::key(archive)?;
            let bytes = cdn.index(key, &data.join("indices"))?;
            let found = formats::archive_index(&bytes, archive, &needed)?;
            for key in found.keys() {
                needed.remove(key);
            }
            locations.extend(found);
            if needed.is_empty() {
                break;
            }
        }
    }
    for item in items.values_mut() {
        item.location = locations.get(&item.key).cloned();
    }
    let storage_keys = items.keys().copied().collect();
    // Loose-file locations are retained alongside storage items for downloads.
    for file in &loose {
        items.entry(file.key).or_insert(Item {
            key: file.key,
            size: 0,
            full: false,
            raw: false,
            location: locations.get(&file.key).cloned(),
        });
    }
    let mut lines: Vec<String> = installed.lines().map(str::to_owned).collect();
    let header: Vec<_> = lines[0]
        .split('|')
        .map(|s| s.split('!').next().unwrap().to_owned())
        .collect();
    for line in lines.iter_mut().skip(1) {
        let mut cells: Vec<_> = line.split('|').map(str::to_owned).collect();
        let get = |name: &str| header.iter().position(|s| s == name);
        if get("Branch").and_then(|i| cells.get(i)).map(String::as_str)
            == current.get("Branch").map(String::as_str)
            && get("Product")
                .and_then(|i| cells.get(i))
                .map(String::as_str)
                == current.get("Product").map(String::as_str)
            && get("Active").and_then(|i| cells.get(i)).map(String::as_str) == Some("1")
        {
            for (name, value) in [
                ("Build Key", formats::hex(&build)),
                ("CDN Key", formats::hex(&cdn_key)),
                ("Version", version.clone()),
                ("Tags", tags.clone()),
            ] {
                if let Some(i) = get(name) {
                    if let Some(cell) = cells.get_mut(i) {
                        *cell = value;
                    }
                }
            }
            *line = cells.join("|");
        }
    }
    let updated = (lines.join("\n") + "\n").into_bytes();
    Ok(Plan {
        root,
        data,
        original,
        updated,
        version,
        build,
        cdn_key,
        build_config,
        cdn_config,
        storage,
        storage_keys,
        items: items.into_values().collect(),
        loose,
        cdn,
    })
}
fn download_all(
    plan: &Plan,
    cancel: &AtomicBool,
    progress: &(impl Fn(Progress) + Sync),
) -> Result<()> {
    let mut archived = Vec::new();
    let mut jobs: Vec<Vec<usize>> = Vec::new();
    for (i, item) in plan.items.iter().enumerate() {
        if item.location.is_some() && !item.raw {
            archived.push(i);
        } else {
            jobs.push(vec![i]);
        }
    }
    archived.sort_by_key(|i| {
        let l = plan.items[*i].location.as_ref().unwrap();
        (l.archive.clone(), l.offset)
    });
    let mut group = Vec::new();
    let mut archive = String::new();
    let mut start = 0;
    let mut end = 0;
    for i in archived {
        let l = plan.items[i].location.as_ref().unwrap();
        if !group.is_empty()
            && (l.archive != archive
                || l.offset > end + 65536
                || l.offset + l.size - start > 32 * 1024 * 1024)
        {
            jobs.push(std::mem::take(&mut group));
        }
        if group.is_empty() {
            archive = l.archive.clone();
            start = l.offset;
            end = l.offset;
        }
        end = end.max(l.offset + l.size);
        group.push(i);
    }
    if !group.is_empty() {
        jobs.push(group);
    }
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let count = plan.items.len();
    let mut error = None;
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        for worker in 0..6.min(jobs.len()) {
            let tx = tx.clone();
            let next = &next;
            let failed = &failed;
            let jobs = &jobs;
            scope.spawn(move || {
                loop {
                    if failed.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else {
                        break;
                    };
                    let result = if job.len() == 1 {
                        plan.cdn.file(&plan.items[job[0]], cancel).map(|_| 1)
                    } else {
                        download_batch(plan, job, cancel, worker)
                    };
                    if result.is_err() {
                        failed.store(true, Ordering::Relaxed);
                    }
                    let _ = tx.send(result);
                }
            });
        }
        drop(tx);
        let mut completed = 0;
        for result in rx {
            match result {
                Ok(n) => {
                    completed += n;
                    progress(Progress {
                        message: format!("Downloading verified game files ({completed}/{count})…"),
                        completed,
                        total: count as u64,
                    });
                }
                Err(e) => {
                    if error.is_none() {
                        error = Some(e);
                    }
                }
            }
        }
    });
    cancelled(cancel)?;
    if let Some(error) = error {
        return Err(error);
    }
    Ok(())
}
fn download_batch(plan: &Plan, job: &[usize], cancel: &AtomicBool, worker: usize) -> Result<u64> {
    use std::io::{Seek, SeekFrom};
    let mut missing = Vec::new();
    for i in job {
        let item = &plan.items[*i];
        let path = plan
            .cdn
            .cache
            .join(format!("{}.encoded", formats::hex(&item.key)));
        if !path.is_file() || formats::verify_file(&path, item.key).is_err() {
            missing.push(*i);
        }
    }
    if missing.is_empty() {
        return Ok(job.len() as u64);
    }
    if missing.len() == 1 {
        plan.cdn.file(&plan.items[missing[0]], cancel)?;
        return Ok(job.len() as u64);
    }
    let first = plan.items[missing[0]].location.as_ref().unwrap();
    let end = missing
        .iter()
        .map(|i| {
            let l = plan.items[*i].location.as_ref().unwrap();
            l.offset + l.size
        })
        .max()
        .unwrap();
    let range = Location {
        archive: first.archive.clone(),
        offset: first.offset,
        size: end - first.offset,
    };
    let temporary = plan
        .cdn
        .cache
        .join(format!("batch-{}-{worker}.part", std::process::id()));
    let mut response = plan
        .cdn
        .request("data", plan.items[missing[0]].key, "", Some(&range))?;
    let mut output = File::create(&temporary).map_err(|e| e.to_string())?;
    let mut length = 0;
    let mut bytes = [0; 65536];
    loop {
        cancelled(cancel)?;
        let n = response.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        length += n as u64;
        if length > range.size {
            return Err("CDN archive range exceeded its expected length.".into());
        }
        output.write_all(&bytes[..n]).map_err(|e| e.to_string())?;
    }
    if length != range.size {
        return Err("CDN archive range was truncated.".into());
    }
    drop(output);
    let mut input = File::open(&temporary).map_err(|e| e.to_string())?;
    for i in missing {
        cancelled(cancel)?;
        let item = &plan.items[i];
        let location = item.location.as_ref().unwrap();
        input
            .seek(SeekFrom::Start(location.offset - range.offset))
            .map_err(|e| e.to_string())?;
        let path = plan
            .cdn
            .cache
            .join(format!("{}.encoded", formats::hex(&item.key)));
        let part = path.with_extension(format!("{}.part", std::process::id()));
        let mut file = File::create(&part).map_err(|e| e.to_string())?;
        let n = std::io::copy(&mut (&mut input).take(location.size), &mut file)
            .map_err(|e| e.to_string())?;
        if n != location.size {
            return Err("Extracted CDN file was truncated.".into());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        formats::verify_file(&part, item.key)?;
        fs::rename(part, path).map_err(|e| e.to_string())?;
    }
    let _ = fs::remove_file(temporary);
    Ok(job.len() as u64)
}
fn sync_directories(mut directory: &Path, root: &Path) -> Result<()> {
    while directory.starts_with(root) {
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())?;
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent;
    }
    Ok(())
}
fn commit_loose(plan: &Plan, staging: &Path) -> Result<()> {
    transaction::begin(plan, staging)?;
    let result = (|| {
        for file in &plan.loose {
            let target = destination(&plan.root, &file.relative)?;
            let staged = staging.join("loose").join(&file.relative);
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            if target.exists() {
                let backup = staging.join("backup").join(&file.relative);
                fs::create_dir_all(backup.parent().unwrap()).map_err(|e| e.to_string())?;
                fs::rename(&target, &backup).map_err(|e| e.to_string())?;
                sync_directories(backup.parent().unwrap(), &plan.root)?;
            }
            fs::rename(staged, &target).map_err(|e| e.to_string())?;
            sync_directories(target.parent().unwrap(), &plan.root)?;
        }
        atomic_write(&plan.root.join(".build.info"), &plan.updated)
    })();
    if let Err(error) = result {
        return match transaction::recover(&plan.root) {
            Ok(_) => Err(format!("{error}. Original executable files were restored.")),
            Err(recovery) => Err(format!(
                "{error}. Recovery files were preserved: {recovery}"
            )),
        };
    }
    transaction::finish(&plan.root)
}
fn apply(
    mut plan: Plan,
    game: &Game,
    cancel: &AtomicBool,
    progress: &(impl Fn(Progress) + Sync),
) -> Result<Summary> {
    let summary = Summary {
        version: plan.version.clone(),
        storage_files: plan
            .items
            .iter()
            .filter(|i| plan.storage_keys.contains(&i.key))
            .count(),
        loose_files: plan.loose.len(),
        download_bytes: plan
            .items
            .iter()
            .map(|i| i.location.as_ref().map(|l| l.size).unwrap_or(i.size))
            .sum(),
    };
    if plan.items.is_empty() && plan.loose.is_empty() && plan.original == plan.updated {
        message(progress, format!("{} is up to date.", game.name));
        return Ok(summary);
    }
    download_all(&plan, cancel, progress)?;
    cancelled(cancel)?;
    let directory = destination(&plan.root, Path::new(".bnet-linux-update"))?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let staging = directory.join(format!("{}-{unique}", formats::hex(&plan.build)));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    for (i, file) in plan.loose.iter().enumerate() {
        cancelled(cancel)?;
        message(
            progress,
            format!(
                "Verifying executable files ({}/{})…",
                i + 1,
                plan.loose.len()
            ),
        );
        let encoded = fs::read(
            plan.cdn
                .cache
                .join(format!("{}.encoded", formats::hex(&file.key))),
        )
        .map_err(|e| e.to_string())?;
        let decoded = formats::blte(&encoded, file.key)?;
        if decoded.len() as u64 != file.file.size || formats::hash(&decoded) != file.file.content {
            return Err(format!(
                "Content verification failed for {}.",
                file.file.path
            ));
        }
        let path = staging.join("loose").join(&file.relative);
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        atomic_write(&path, &decoded)?;
        if let Ok(metadata) = fs::metadata(plan.root.join(&file.relative)) {
            fs::set_permissions(&path, metadata.permissions()).map_err(|e| e.to_string())?;
        }
    }
    for item in &plan.items {
        cancelled(cancel)?;
        if !plan.storage_keys.contains(&item.key) {
            continue;
        }
        message(progress, "Preparing CASC content…");
        let path = plan
            .cdn
            .cache
            .join(format!("{}.encoded", formats::hex(&item.key)));
        plan.storage.append_file(item.key, &path, item.full)?;
    }
    cancelled(cancel)?;
    if runner::game_running(game) {
        return Err("The game started while files were being prepared. Close it and retry; the installed build was not changed.".into());
    }
    if updater_running(game) {
        return Err(
            "The Windows update agent started during patch preparation. Close it and retry.".into(),
        );
    }
    if fs::read(plan.root.join(".build.info")).map_err(|e| e.to_string())? != plan.original {
        return Err("Another updater changed this installation. Refresh before retrying.".into());
    }
    message(progress, "Applying verified patch…");
    for (key, bytes) in [
        (plan.build, &plan.build_config),
        (plan.cdn_key, &plan.cdn_config),
    ] {
        let hash = formats::hex(&key);
        let path = plan
            .data
            .join("config")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(&hash);
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        atomic_write(&path, bytes)?;
    }
    // Cache-only downloads and staging are complete before indexes or loose
    // files change. Keep old archive/index generations for rollback/resume.
    if !plan.storage_keys.is_empty() {
        plan.storage.commit()?;
    }
    commit_loose(&plan, &staging)?;
    let _ = fs::remove_dir_all(&staging);
    message(
        progress,
        format!("{} {} is ready.", game.name, summary.version),
    );
    Ok(summary)
}
fn updater_running(game: &Game) -> bool {
    let Ok(processes) = fs::read_dir("/proc") else {
        return false;
    };
    let expected_prefix = fs::canonicalize(&game.prefix).ok();
    processes.flatten().any(|p| {
        if !p
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            return false;
        }
        let path = p.path();
        let Ok(bytes) = fs::read(path.join("cmdline")) else {
            return false;
        };
        let first = bytes.split(|b| *b == 0).next().unwrap_or_default();
        let name = String::from_utf8_lossy(first).replace('\\', "/");
        if !name.rsplit('/').next().is_some_and(|name| {
            name.eq_ignore_ascii_case("Agent.exe") || name.eq_ignore_ascii_case("Battle.net.exe")
        }) {
            return false;
        }
        // Inspect only the prefix selector and never log process environments.
        fs::read(path.join("environ")).is_ok_and(|bytes| {
            bytes.split(|b| *b == 0).any(|value| {
                value.strip_prefix(b"WINEPREFIX=").is_some_and(|prefix| {
                    use std::os::unix::ffi::OsStrExt;
                    let actual =
                        fs::canonicalize(Path::new(std::ffi::OsStr::from_bytes(prefix))).ok();
                    actual.is_some() && actual == expected_prefix
                })
            })
        })
    })
}
pub fn recover_installation(game: &Game) -> Result<()> {
    let root = crate::catalog::target_root(game)?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !root.join(".bnet-linux-update/transaction.json").exists() {
        return Ok(());
    }
    if runner::game_running(game) {
        return Err("Close the game before recovering an interrupted patch.".into());
    }
    let lease = OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join(".bnet-linux-update.lock"))
        .map_err(|e| e.to_string())?;
    lease
        .try_lock()
        .map_err(|_| "An updater is currently modifying the installation.")?;
    transaction::recover(&root)?;
    Ok(())
}
pub fn run(
    game: &Game,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(Progress) + Sync,
    plan_only: bool,
) -> Result<Summary> {
    run_mode(game, cancel, progress, plan_only, false)
}
pub fn repair(
    game: &Game,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(Progress) + Sync,
) -> Result<Summary> {
    run_mode(game, cancel, progress, false, true)
}
fn run_mode(
    game: &Game,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(Progress) + Sync,
    plan_only: bool,
    repair: bool,
) -> Result<Summary> {
    let _lease = if plan_only {
        None
    } else {
        if runner::game_running(game) {
            return Err("Close the game before patching its files.".into());
        }
        if updater_running(game) {
            return Err(
                "Close the Windows Battle.net client and its update agent before native patching."
                    .into(),
            );
        }
        let root = crate::catalog::target_root(game)?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(".bnet-linux-update.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "Another native updater is already patching this installation.")?;
        Some(file)
    };
    if !plan_only {
        install::prepare(game, &cancel)?;
        transaction::recover(
            &crate::catalog::target_root(game)?
                .canonicalize()
                .map_err(|e| e.to_string())?,
        )?;
    }
    let plan = plan(game, &cancel, &progress, repair)?;
    if plan_only {
        return Ok(Summary {
            version: plan.version,
            storage_files: plan.storage_keys.len(),
            loose_files: plan.loose.len(),
            download_bytes: plan
                .items
                .iter()
                .map(|i| i.location.as_ref().map(|l| l.size).unwrap_or(i.size))
                .sum(),
        });
    }
    apply(plan, game, &cancel, &progress)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_manifest_paths_that_escape_installation() {
        for path in [
            "../file",
            "/file",
            "C:/file",
            "dir/../file",
            "dir\\file",
            "dir//file",
        ] {
            assert!(relative(path).is_err());
        }
        assert_eq!(
            relative("Support64/SC2Switcher_x64.exe").unwrap(),
            PathBuf::from("Support64/SC2Switcher_x64.exe")
        );
    }
}
/// Integration exercise against real public manifests in a separate sparse
/// fixture. Source archives are read only; no hard links or game processes.
#[cfg(debug_assertions)]
pub fn exercise(game: &Game, directory: &Path, cancel_apply: bool) -> Result<()> {
    let product = crate::catalog::product(&game.product_code)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let log = |p: Progress| println!("{}", p.message);
    let source = plan(game, &cancel, &log, false)?;
    if !source.items.is_empty() || !source.loose.is_empty() {
        return Err("Update the source installation before exercising a sparse fixture.".into());
    }
    if directory.exists() {
        return Err("The native update fixture destination must not already exist.".into());
    }
    let parent = directory
        .parent()
        .ok_or("Missing fixture parent.")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if parent.starts_with(&source.root) {
        return Err("A patch fixture cannot be inside the source installation.".into());
    }
    let config = formats::config(&source.build_config)?;
    let (content, encoded) = pair(&config, "install")?;
    let rows = formats::table(std::str::from_utf8(&source.original).map_err(|e| e.to_string())?)?;
    let current = rows
        .iter()
        .find(|r| r.get("Active").map(String::as_str) == Some("1"))
        .ok_or("No active source build.")?;
    let tags = field(current, "Tags")?;
    let files = formats::install(
        &source
            .cdn
            .decoded(encoded, Some(content), &source.storage, &cancel)?,
        tags,
    )?;
    let missing = files
        .iter()
        .filter(|f| f.size > 0 && f.size < 128 * 1024 && !f.path.ends_with("SC2Switcher_x64.exe"))
        .min_by_key(|f| f.size)
        .ok_or("No small loose file available for fixture.")?
        .clone();
    let (content, encoded) = pair(&config, "download")?;
    let download = formats::download(
        &source
            .cdn
            .decoded(encoded, Some(content), &source.storage, &cancel)?,
        tags,
    )?;
    let changed = download
        .into_iter()
        .find(|(key, size)| *size > 32 && *size < 64 * 1024 && source.storage.contains(key))
        .ok_or("No small CASC file available for fixture.")?
        .0;
    let original_blob = source.storage.read(&changed)?;
    let original_loose = hash_file(&source.root.join(relative(&missing.path)?))?;
    fs::create_dir(directory).map_err(|e| e.to_string())?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    message(&log, "Creating isolated sparse CASC fixture…");
    let data = directory.join(crate::catalog::product(&game.product_code)?.data_dir);
    fs::create_dir_all(data.join("data")).map_err(|e| e.to_string())?;
    fn copy_tree(from: &Path, to: &Path) -> Result<()> {
        fs::create_dir_all(to).map_err(|e| e.to_string())?;
        for e in fs::read_dir(from).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let metadata = e.file_type().map_err(|e| e.to_string())?;
            if metadata.is_symlink() {
                return Err("A fixture source contains a symbolic link.".into());
            }
            if metadata.is_dir() {
                copy_tree(&e.path(), &to.join(e.file_name()))?;
            } else {
                fs::copy(e.path(), to.join(e.file_name())).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    for name in ["config", "indices"] {
        copy_tree(&source.data.join(name), &data.join(name))?;
    }
    for e in fs::read_dir(source.data.join("data")).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name
            .strip_prefix("data.")
            .and_then(|s| s.parse::<u16>().ok())
            .is_some()
        {
            let mut input = File::open(e.path()).map_err(|e| e.to_string())?;
            let output = data.join("data").join(name.as_ref());
            let mut output = File::create(output).map_err(|e| e.to_string())?;
            output
                .set_len(input.metadata().map_err(|e| e.to_string())?.len())
                .map_err(|e| e.to_string())?;
            let mut prefix = [0; 480];
            input.read_exact(&mut prefix).map_err(|e| e.to_string())?;
            output.write_all(&prefix).map_err(|e| e.to_string())?;
        } else if name.ends_with(".idx") || name == "shmem" {
            fs::copy(e.path(), data.join("data").join(name.as_ref())).map_err(|e| e.to_string())?;
        }
    }
    // Only the three manifests read during planning need actual old payloads.
    use std::io::{Seek, SeekFrom};
    for name in ["install", "download", "encoding"] {
        let (_, key) = pair(&config, name)?;
        let entry = source
            .storage
            .entries
            .get(&key[..9])
            .ok_or("Missing metadata in source storage.")?;
        let file = format!("data.{:03}", entry.location >> 30);
        let mut input =
            File::open(source.data.join("data").join(&file)).map_err(|e| e.to_string())?;
        let mut output = OpenOptions::new()
            .write(true)
            .open(data.join("data").join(&file))
            .map_err(|e| e.to_string())?;
        let offset = entry.location & ((1 << 30) - 1);
        input
            .seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        output
            .seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input.take(entry.size as u64), &mut output)
            .map_err(|e| e.to_string())?;
    }
    for file in &files {
        let mut relative = relative(&file.path)?;
        if !product.containerless
            && relative.components().next().is_some_and(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(product.data_dir.split('/').next().unwrap_or("Data"))
            })
        {
            return Err("Install manifest attempts to overwrite CASC storage.".into());
        }
        if !product.game_dir.is_empty() && !relative.starts_with(product.game_dir) {
            relative = PathBuf::from(product.game_dir).join(relative);
        }
        let target = destination(&directory, &relative)?;
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::copy(source.root.join(relative), target).map_err(|e| e.to_string())?;
    }
    let mut marker = String::from_utf8(source.original.clone()).map_err(|e| e.to_string())?;
    marker = marker.replace(&source.version, "0.0.0.1");
    fs::write(directory.join(".build.info"), marker.as_bytes()).map_err(|e| e.to_string())?;
    fs::remove_file(directory.join(relative(&missing.path)?)).map_err(|e| e.to_string())?;
    let mut storage = Storage::open(data.join("data"))?;
    storage.entries.remove(&changed[..9]);
    storage.commit()?;
    drop(storage);
    let fixture = Game {
        executable: directory
            .join(crate::catalog::product(&game.product_code)?.executable)
            .to_string_lossy()
            .into(),
        prefix: directory.join("unused-prefix").to_string_lossy().into(),
        ..game.clone()
    };
    let plan = plan(&fixture, &cancel, &log, false)?;
    if plan.storage_keys.is_empty() || plan.loose.is_empty() {
        return Err("Fixture did not exercise both CASC and loose-file patching.".into());
    }
    let result = apply(plan, &fixture, &cancel, &|p: Progress| {
        if cancel_apply && p.message.starts_with("Verifying executable files") {
            cancel.store(true, Ordering::Relaxed);
        }
        log(p);
    });
    if cancel_apply {
        if result.is_ok()
            || fs::read(directory.join(".build.info")).map_err(|e| e.to_string())?
                != marker.as_bytes()
            || directory.join(relative(&missing.path)?).exists()
        {
            return Err("Cancellation changed the fixture's active build.".into());
        }
        println!("PASS: cancelled patch leaves original build metadata and loose files unchanged");
    } else {
        let summary = result?;
        println!(
            "Applied {}: {} storage files, {} loose files",
            summary.version, summary.storage_files, summary.loose_files
        );
        let storage = Storage::open(data.join("data"))?;
        if storage.read(&changed)? != original_blob {
            return Err("Patched CASC payload does not match the verified source content.".into());
        }
        if hash_file(&directory.join(relative(&missing.path)?))? != missing.content {
            return Err("Patched loose file does not match Blizzard's content hash.".into());
        }
        if fs::read(directory.join(".build.info")).map_err(|e| e.to_string())? != source.original {
            return Err("Fixture did not commit the expected installed build.".into());
        }
        println!(
            "PASS: native CDN download, CASC append/index/shmem commit, loose-file replacement, and final build activation"
        );
    }
    if fs::read(source.root.join(".build.info")).map_err(|e| e.to_string())? != source.original
        || source.storage.read(&changed)? != original_blob
        || hash_file(&source.root.join(relative(&missing.path)?))? != original_loose
    {
        return Err("The source installation changed during the fixture test.".into());
    }
    println!("PASS: source game installation is unchanged");
    Ok(())
}
