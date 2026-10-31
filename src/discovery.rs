use crate::{
    catalog,
    runner::{self, Game},
};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[derive(Clone, Copy)]
pub enum Pick {
    GameFolder,
    GameExe,
    Wine,
    Prefix,
    Proton,
    Umu,
}
impl Pick {
    pub fn folder(self) -> bool {
        matches!(self, Self::GameFolder | Self::Prefix | Self::Proton)
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::GameFolder => "Choose game installation folder",
            Self::GameExe => "Locate game using an executable",
            Self::Wine => "Choose Wine executable or wrapper",
            Self::Prefix => "Choose existing Wine prefix",
            Self::Proton => "Choose Proton installation",
            Self::Umu => "Choose umu-run executable",
        }
    }
}
pub fn pick(kind: Pick, current: &str) -> Result<Option<PathBuf>, String> {
    let helper = runner::login_helper().ok_or("The desktop picker component is missing.")?;
    let mut command = Command::new(helper);
    runner::system_environment(&mut command);
    let output = command
        .arg("--choose-path")
        .arg(if kind.folder() { "folder" } else { "file" })
        .arg(kind.title())
        .arg(current)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("The desktop file picker could not open.".into());
    }
    let choice: Option<String> =
        serde_json::from_slice(&output.stdout).map_err(|_| "Invalid desktop picker response.")?;
    Ok(choice.map(PathBuf::from))
}
#[derive(Clone)]
pub struct Runtime {
    pub name: String,
    pub path: PathBuf,
    pub proton: bool,
}
#[derive(Clone)]
pub struct Installation {
    pub root: PathBuf,
    pub executable: PathBuf,
    pub prefix: Option<PathBuf>,
}
#[derive(Default)]
pub struct Found {
    pub runtimes: Vec<Runtime>,
    pub installations: Vec<Installation>,
    pub prefixes: Vec<PathBuf>,
    pub umu: Option<PathBuf>,
}
fn executable(path: &Path) -> bool {
    path.is_file() && fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}
pub fn resolve(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut result = root.to_owned();
    for component in relative.split('/') {
        let direct = result.join(component);
        if direct.exists() {
            result = direct;
        } else {
            result = fs::read_dir(&result)
                .ok()?
                .flatten()
                .find(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(component))
                })?
                .path();
        }
    }
    Some(result)
}
fn prefix(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .take(10)
        .find(|p| p.join("system.reg").is_file())
        .map(Path::to_owned)
}
fn installation(root: &Path, code: &str) -> Option<Installation> {
    let product = catalog::product(code).ok()?;
    let exe = resolve(root, product.executable)?;
    if !exe.is_file() {
        return None;
    }
    let root = root.canonicalize().ok()?;
    Some(Installation {
        prefix: prefix(&root),
        root,
        executable: exe,
    })
}
pub fn locate(path: &Path, code: &str) -> Option<Installation> {
    let start = if path.is_file() { path.parent()? } else { path };
    for root in start.ancestors().take(7) {
        if let Some(game) = installation(root, code) {
            return Some(game);
        }
    }
    let mut queue = VecDeque::from([(start.to_owned(), 0)]);
    let mut count = 0;
    while let Some((root, depth)) = queue.pop_front() {
        count += 1;
        if count > 200 {
            break;
        }
        if let Some(game) = installation(&root, code) {
            return Some(game);
        }
        if depth < 2 {
            for e in fs::read_dir(&root)
                .into_iter()
                .flatten()
                .flatten()
                .take(100)
            {
                if e.file_type().is_ok_and(|t| t.is_dir())
                    && !skip(&e.file_name().to_string_lossy())
                {
                    queue.push_back((e.path(), depth + 1));
                }
            }
        }
    }
    None
}
fn skip(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "data"
            | "sc2data"
            | "heroesdata"
            | "windows"
            | "programdata"
            | "node_modules"
            | ".git"
            | "cache"
            | "logs"
            | "versions"
            | "screenshots"
    ) || name.starts_with(".bnet-linux-")
}
fn host_program(name: &str) -> Option<PathBuf> {
    let out = runner::host_command("which")
        .arg(name)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8(out.stdout).ok()?;
    let path = PathBuf::from(value.trim());
    path.is_absolute().then_some(path)
}
pub fn scan(game: &Game) -> Found {
    let Some(user) = directories::UserDirs::new() else {
        return Found::default();
    };
    let home = user.home_dir();
    let mut found = Found::default();
    for name in ["wine", "wine64"] {
        if let Some(path) = host_program(name) {
            found.runtimes.push(Runtime {
                name: format!("System {name}"),
                path,
                proton: false,
            });
        }
    }
    found.umu = host_program("umu-run").or_else(|| {
        let p = home.join(".local/share/lutris/runtime/umu/umu-run");
        executable(&p).then_some(p)
    });
    let mut runner_roots = vec![
        home.join(".local/share/lutris/runners/wine"),
        home.join(".local/share/Steam/compatibilitytools.d"),
        home.join(".steam/root/compatibilitytools.d"),
        home.join(".local/share/Steam/steamapps/common"),
        home.join(".local/share/umu/compatibilitytools"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps/common"),
    ];
    for steam in [
        home.join(".local/share/Steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ] {
        if let Ok(text) = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
            for line in text.lines().filter(|s| s.contains("\"path\"")) {
                let parts: Vec<_> = line.split('"').collect();
                if let Some(path) = parts.get(3) {
                    let root = PathBuf::from(path.replace("\\\\", "\\"));
                    if root.is_absolute() {
                        runner_roots.push(root.join("steamapps/common"));
                    }
                }
            }
        }
    }
    for root in runner_roots {
        for entry in fs::read_dir(root).into_iter().flatten().flatten().take(250) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if executable(&path.join("proton")) {
                found.runtimes.push(Runtime {
                    name,
                    path,
                    proton: true,
                });
            } else {
                for binary in ["bin/wine", "bin/wine64"] {
                    let path = path.join(binary);
                    if executable(&path) {
                        found.runtimes.push(Runtime {
                            name: name.clone(),
                            path,
                            proton: false,
                        });
                        break;
                    }
                }
            }
        }
    }
    let mut seen = HashSet::new();
    found.runtimes.retain(|r| {
        seen.insert((
            r.path.canonicalize().unwrap_or_else(|_| r.path.clone()),
            r.proton,
        ))
    });
    found
        .runtimes
        .sort_by_key(|r| (r.proton, r.name.to_lowercase()));
    let mut roots = vec![
        home.join("Games"),
        home.join(".wine"),
        home.join(".local/share/lutris/prefixes"),
        home.join(".local/share/bottles/bottles"),
        home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
        home.join(".local/share/Steam/steamapps/compatdata"),
        home.join(".steam/root/steamapps/compatdata"),
    ];
    if !game.prefix.is_empty() {
        roots.push(PathBuf::from(&game.prefix));
    }
    if let Ok(root) = catalog::target_root(game) {
        roots.push(root);
    }
    let mut queue: VecDeque<_> = roots
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|p| (p, 0))
        .collect();
    let mut seen = HashSet::new();
    let mut installs = HashSet::new();
    let mut prefixes = HashSet::new();
    while let Some((root, depth)) = queue.pop_front() {
        if seen.len() >= 4000 {
            break;
        }
        let Ok(canonical) = root.canonicalize() else {
            continue;
        };
        if !seen.insert(canonical.clone()) {
            continue;
        }
        if root.join("system.reg").is_file() && prefixes.insert(canonical.clone()) {
            found.prefixes.push(canonical.clone());
        }
        if let Some(game) = installation(&root, &game.product_code) {
            if installs.insert(game.root.clone()) {
                found.installations.push(game);
            }
        }
        if depth >= 6 {
            continue;
        }
        for entry in fs::read_dir(&root)
            .into_iter()
            .flatten()
            .flatten()
            .take(200)
        {
            if entry.file_type().is_ok_and(|t| t.is_dir())
                && !skip(&entry.file_name().to_string_lossy())
            {
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }
    found.installations.sort_by_key(|i| i.root.clone());
    found.prefixes.sort();
    found
}
pub fn apply(game: &mut Game, kind: Pick, path: &Path) -> Result<String, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let text = path
        .to_str()
        .ok_or("Paths must be valid UTF-8.")?
        .to_owned();
    match kind {
        Pick::GameFolder | Pick::GameExe => {
            if let Some(install) = locate(&path, &game.product_code) {
                game.install_dir = install.root.to_string_lossy().into();
                game.executable = install.executable.to_string_lossy().into();
                if game.prefix.is_empty() {
                    if let Some(prefix) = install.prefix {
                        game.prefix = prefix.to_string_lossy().into();
                    }
                }
                return Ok(format!(
                    "Located {}. Its supported launcher will be used.",
                    game.name
                ));
            }
            if matches!(kind, Pick::GameExe) {
                return Err(format!(
                    "Could not locate a supported {} installation around that executable.",
                    game.name
                ));
            }
            let product = catalog::product(&game.product_code)?;
            game.install_dir = text;
            game.executable = path.join(product.executable).to_string_lossy().into();
            Ok("Installation folder selected. No existing game was detected there.".into())
        }
        Pick::Prefix => {
            if !path.join("system.reg").is_file() {
                return Err(
                    "Choose the Wine prefix itself: the folder containing system.reg and drive_c."
                        .into(),
                );
            }
            game.prefix = text;
            Ok("Wine prefix selected.".into())
        }
        Pick::Proton => {
            if !executable(&path.join("proton")) {
                return Err(
                    "Choose the Proton folder containing the executable proton script.".into(),
                );
            }
            game.proton = text;
            game.runtime_kind = "proton".into();
            Ok("Proton runner selected.".into())
        }
        Pick::Wine | Pick::Umu => {
            if !executable(&path) {
                return Err("Choose a Linux executable or executable wrapper script.".into());
            }
            if matches!(kind, Pick::Wine) {
                game.runtime = text;
                game.runtime_kind = "wine".into();
            } else {
                game.umu = text;
            }
            Ok("Runtime executable selected.".into())
        }
    }
}
