use crate::auth::{Account, Ticket};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Game {
    pub name: String,
    pub product_code: String,
    pub executable: String,
    pub install_dir: String,
    pub runtime: String,
    pub runtime_kind: String,
    pub prefix: String,
    pub region: String,
    pub locale: String,
    pub umu: String,
    pub proton: String,
    pub auto_update: bool,
}
impl Default for Game {
    fn default() -> Self {
        Self {
            name: "StarCraft II".into(),
            product_code: "S2".into(),
            executable: String::new(),
            install_dir: String::new(),
            runtime: "wine".into(),
            runtime_kind: "auto".into(),
            prefix: String::new(),
            region: "EU".into(),
            locale: "enUS".into(),
            umu: String::new(),
            proton: String::new(),
            auto_update: true,
        }
    }
}
fn bridge_path() -> Result<PathBuf, String> {
    let path = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")
        .ok_or("Cannot resolve application data directory.")?
        .data_local_dir()
        .join("bnet-session-bridge-v1.exe");
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let embedded = include_bytes!("../assets/bnet-session-bridge.exe");
    if fs::read(&path).ok().as_deref() != Some(embedded.as_slice()) {
        let staging = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&staging, embedded).map_err(|e| e.to_string())?;
        fs::rename(&staging, &path).map_err(|e| e.to_string())?;
    }
    Ok(path)
}
fn system_data_dirs(value: &str) -> String {
    let paths: Vec<_> = value
        .split(':')
        .filter(|path| !path.is_empty() && !path.contains("/snap/"))
        .collect();
    if paths.is_empty() {
        "/usr/local/share:/usr/share".into()
    } else {
        paths.join(":")
    }
}
pub(crate) fn system_environment(command: &mut Command) {
    // The native system GTK/Wine processes must not consume the editor's Snap schemas.
    for key in [
        "GIO_MODULE_DIR",
        "GTK_PATH",
        "GTK_MODULES",
        "GSETTINGS_SCHEMA_DIR",
    ] {
        command.env_remove(key);
    }
    if let Ok(value) = std::env::var("XDG_DATA_DIRS") {
        command.env("XDG_DATA_DIRS", system_data_dirs(&value));
    }
}
pub fn check_login_window() -> Result<(), String> {
    let helper = login_helper().ok_or("The login component is missing.")?;
    let mut command = Command::new(helper);
    system_environment(&mut command);
    let output = command
        .args(["EU", "--startup-check"])
        .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() && output.stdout == b"ready\n" {
        Ok(())
    } else {
        Err(format!("Login window startup failed: {}", output.status))
    }
}
pub fn host_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    if std::env::var_os("FLATPAK_ID").is_some() {
        let mut command = Command::new("/app/bin/bnet-host-run");
        command.arg(program);
        command
    } else {
        Command::new(program)
    }
}
fn runtime_command(game: &Game) -> Result<Command, String> {
    let prefix = Path::new(&game.prefix);
    if !prefix.is_absolute() || !prefix.join("system.reg").is_file() {
        return Err(
            "Choose an existing Wine prefix in Settings (the directory containing system.reg)."
                .into(),
        );
    }
    if game.runtime.trim().is_empty() {
        return Err("Choose a Wine runtime in Settings.".into());
    }
    if !matches!(game.region.as_str(), "EU" | "US" | "KR" | "CN")
        || game.locale.len() != 4
        || !game.locale.bytes().all(|b| b.is_ascii_alphabetic())
    {
        return Err("Invalid region or locale.".into());
    }
    let mut cmd = if !uses_proton(game) {
        host_command(&game.runtime)
    } else {
        if !Path::new(&game.umu).is_file() || !Path::new(&game.proton).join("proton").is_file() {
            return Err(
                "Choose an existing umu-run executable and Proton directory in Settings.".into(),
            );
        }
        let mut cmd = host_command(&game.umu);
        cmd.env("PROTONPATH", &game.proton)
            .env("GAMEID", "battlenet")
            .env("PROTON_VERB", "run");
        cmd
    };
    cmd.env("WINEPREFIX", prefix)
        .env("WINEDEBUG", "-all")
        .env_remove("GIO_MODULE_DIR")
        .env_remove("GTK_PATH")
        .env_remove("GTK_MODULES")
        .stdin(Stdio::piped())
        .stderr(Stdio::null());
    system_environment(&mut cmd);
    Ok(cmd)
}
fn command(game: &Game) -> Result<Command, String> {
    let mut command = runtime_command(game)?;
    let helper = bridge_path()?;
    command.arg(&helper);
    if uses_proton(game) {
        command.env("STEAM_COMPAT_INSTALL_PATH", helper.parent().unwrap());
    }
    Ok(command)
}
fn private_file(name: &str) -> Result<File, String> {
    private_file_at(log_path().with_file_name(name))
}
fn game_private_file(game: &Game, name: &str) -> Result<File, String> {
    private_file_at(game_log_path(game).with_file_name(name))
}
fn private_file_at(path: PathBuf) -> Result<File, String> {
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    Ok(file)
}
fn spawn_bridge(
    game: &Game,
    payload: &str,
    output: &str,
) -> Result<(Child, Option<TcpStream>), String> {
    let mut cmd = command(game)?;
    cmd.stderr(game_private_file(
        game,
        if output == "last-launch.log" {
            "runtime.log"
        } else {
            "login-runtime.log"
        },
    )?);
    cmd.stdout(game_private_file(game, output)?)
        .env_remove("BNET_BRIDGE_PORT")
        .env_remove("BNET_BRIDGE_NONCE");
    let network = if !uses_proton(game) {
        None
    } else {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let mut random = [0u8; 32];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut random))
            .map_err(|e| e.to_string())?;
        let nonce = Zeroizing::new(
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        );
        cmd.env(
            "BNET_BRIDGE_PORT",
            listener.local_addr().unwrap().port().to_string(),
        )
        .env("BNET_BRIDGE_NONCE", nonce.as_str());
        Some((listener, nonce))
    };
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Cannot start configured runtime: {e}"))?;
    if let Some((listener, nonce)) = network {
        drop(child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let mut hello = [0u8; 64];
                if stream.read_exact(&mut hello).is_ok() && hello == nonce.as_bytes() {
                    let result = stream
                        .write_all(payload.as_bytes())
                        .and_then(|_| stream.shutdown(Shutdown::Write));
                    if result.is_ok() {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(45)));
                        return Ok((child, Some(stream)));
                    }
                    break;
                }
            }
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        Err(
            "Proton login bridge did not connect. See runtime.log in the launcher cache directory."
                .into(),
        )
    } else {
        if child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Wine login bridge closed unexpectedly.".into());
        }
        Ok((child, None))
    }
}
fn bridge_header(game: &Game) -> Result<String, String> {
    crate::catalog::product(&game.product_code)?;
    Ok(if game.product_code == "S2" {
        "BNET-SC2-1".into()
    } else {
        format!("BNET-GAME-1\n{}", game.product_code)
    })
}
fn exchange(game: &Game, mode: &str, ticket: Option<&Ticket>) -> Result<(), String> {
    let payload = if let Some(ticket) = ticket {
        Zeroizing::new(format!(
            "{}\n{mode}\n{}\n{}\n{}\n\n",
            bridge_header(game)?,
            ticket.expose(),
            game.region,
            game.locale
        ))
    } else {
        Zeroizing::new(format!("{}\n{mode}\n", bridge_header(game)?))
    };
    let response_name = format!("bridge-response-{}.log", std::process::id());
    let response_path = game_log_path(game).with_file_name(&response_name);
    let (mut child, network) = spawn_bridge(game, &payload, &response_name)?;
    if let Some(stream) = network {
        let mut reply = String::new();
        let result = stream.take(256).read_to_string(&mut reply);
        let _ = fs::remove_file(&response_path);
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Proton login bridge response timed out.".into());
        }
        // An authenticated response is sufficient; reap the runtime wrapper separately.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        let expected = match mode {
            "write" => "stored\nexit:87\n",
            "clear" => "cleared\nexit:87\n",
            _ => "ticket-ready\nexit:87\n",
        };
        return if reply == expected {
            Ok(())
        } else if reply == "exit:4\n" {
            Err("No readable game login ticket in this prefix. Sign in to Battle.net.".into())
        } else {
            Err("Proton login bridge rejected the operation. See runtime.log.".into())
        };
    }
    let deadline = Instant::now() + Duration::from_secs(45);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "Wine login bridge timed out. Check the selected runtime and prefix.".into(),
                );
            }
        }
    };
    let response = fs::read(&response_path).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(response_path);
    let expected = match mode {
        "write" => b"stored\n".as_slice(),
        "clear" => b"cleared\n".as_slice(),
        _ => b"ticket-ready\n".as_slice(),
    };
    if status.code() != Some(87) || response != expected {
        return Err(match status.code() {
            Some(4) => "No readable game login ticket in this prefix. Sign in to Battle.net.",
            Some(5) => "Wine could not protect the login ticket with DPAPI.",
            _ => "The Wine login bridge failed. See runtime.log in the launcher cache directory.",
        }
        .into());
    }
    Ok(())
}
pub fn store(game: &Game, ticket: Ticket) -> Result<(), String> {
    exchange(game, "write", Some(&ticket))
}
pub fn login_helper() -> Option<PathBuf> {
    let sibling = std::env::current_exe().ok()?.parent()?.join("bnet-login");
    if cfg!(debug_assertions) {
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        return [
            project.join("login-helper/target/debug/bnet-login"),
            project.join("login-helper/target/release/bnet-login"),
            sibling,
        ]
        .into_iter()
        .filter(|p| p.is_file())
        .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    }
    sibling.is_file().then_some(sibling)
}
fn session_dir() -> Result<PathBuf, String> {
    let path = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")
        .ok_or("Cannot resolve account storage.")?
        .data_local_dir()
        .join("web-session");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let cookies = File::options()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path.join("cookies.sqlite"))
        .map_err(|e| e.to_string())?;
    cookies
        .set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    Ok(path)
}
fn run_login(
    helper: &Path,
    region: &str,
    product: Option<&str>,
) -> Result<Zeroizing<Vec<u8>>, String> {
    run_login_start(helper, region, product, None, false)
}
fn run_login_start(
    helper: &Path,
    region: &str,
    product: Option<&str>,
    start: Option<&str>,
    check_session: bool,
) -> Result<Zeroizing<Vec<u8>>, String> {
    use std::os::unix::process::ExitStatusExt;
    let mut version_command = Command::new(helper);
    system_environment(&mut version_command);
    let version = version_command
        .arg("--protocol-version")
        .output()
        .map_err(|e| format!("Could not start the sign-in component: {e}"))?;
    if !version.status.success() || version.stdout != b"bnet-login/2\n" {
        return Err("The sign-in component is out of date. Start the launcher with scripts/run.sh to update both components.".into());
    }
    let session = session_dir()?;
    let lease = File::options()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(session.join("session.lock"))
        .map_err(|e| e.to_string())?;
    lease.try_lock().map_err(|_| {
        "Another sign-in window is already open. Complete or close it first.".to_owned()
    })?;
    let mut cmd = Command::new(helper);
    cmd.arg(region).arg("--session-dir").arg(&session);
    if let Some(product) = product {
        cmd.arg("--game").arg(product);
    } else {
        cmd.arg("--account");
    }
    if check_session {
        cmd.arg("--check-session");
    }
    if start.is_some() {
        cmd.arg("--chat-start-stdin");
    }
    let mut pending = private_file("login-status.log")?;
    let _ = writeln!(
        pending,
        "mode={}\nphase=opening",
        if product == Some("app") {
            "chat"
        } else if product.is_some() {
            "game"
        } else {
            "account"
        }
    );
    drop(pending);
    system_environment(&mut cmd);
    cmd.env_remove("GIO_MODULE_DIR")
        .env_remove("GTK_PATH")
        .env_remove("GTK_MODULES")
        .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        .stdin(if start.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = if let Some(start) = start {
        let mut child = cmd.spawn().map_err(|_| "Could not open chat sign-in")?;
        let write = child
            .stdin
            .take()
            .ok_or("Missing chat sign-in input")?
            .write_all(start.as_bytes());
        if write.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Could not initialize chat sign-in".into());
        }
        child.wait_with_output()
    } else {
        cmd.output()
    }
    .map_err(|_| "Could not open Battle.net sign-in")?;
    let bytes = Zeroizing::new(output.stdout);
    let stderr = Zeroizing::new(output.stderr);
    let known = [
        "window_opened",
        "window_closed",
        "cancelled",
        "display_failed",
        "network_failed",
        "web_process_terminated",
        "callback_rejected",
        "account_profile_failed",
        "account_verified",
    ];
    let events: Vec<&str> = std::str::from_utf8(&stderr)
        .unwrap_or("")
        .lines()
        .filter_map(|line| line.strip_prefix("BNET_EVENT="))
        .filter(|event| known.contains(event))
        .collect();
    let mut log = private_file("login-status.log")?;
    let _ = writeln!(
        log,
        "mode={}\nexit={:?}\nsignal={:?}\nevents={}",
        if product == Some("app") {
            "chat"
        } else if product.is_some() {
            "game"
        } else {
            "account"
        },
        output.status.code(),
        output.status.signal(),
        events.join(",")
    );
    if check_session && output.status.code() == Some(4) {
        return Err("SESSION_EXPIRED".into());
    }
    if !output.status.success() {
        return Err(login_error(
            output.status.code(),
            output.status.signal(),
            &events,
        ));
    }
    Ok(bytes)
}
fn login_error(code: Option<i32>, signal: Option<i32>, events: &[&str]) -> String {
    if let Some(signal) = signal {
        return format!(
            "Battle.net sign-in window crashed (signal {signal}). Open Details for the login diagnostics."
        );
    }
    if code == Some(3) || events.contains(&"display_failed") {
        return "Could not open the sign-in window on this desktop. Open Details for login diagnostics.".into();
    }
    if events.contains(&"web_process_terminated") {
        return "The sign-in browser stopped unexpectedly. Please retry.".into();
    }
    if events.contains(&"callback_rejected") {
        return "Blizzard's game authorization response was rejected. Your account may still be signed in.".into();
    }
    if events.contains(&"account_profile_failed") {
        return "Could not verify the Battle.net account session. Please retry sign-in.".into();
    }
    if events.contains(&"cancelled") {
        return "Sign-in cancelled.".into();
    }
    if events.contains(&"network_failed") {
        return "Could not load Battle.net sign-in. Check the connection and retry.".into();
    }
    if events.contains(&"window_closed") {
        return "The sign-in window was closed before account verification completed.".into();
    }
    format!(
        "The sign-in component exited unexpectedly (code {}). Open Details for login diagnostics.",
        code.map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".into())
    )
}
pub fn remembered_account() -> Option<Account> {
    let directory = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")?;
    let bytes = fs::read(directory.data_local_dir().join("web-session/account.json")).ok()?;
    let account: Account = serde_json::from_slice(&bytes).ok()?;
    account.validate().ok()?;
    Some(account)
}
fn remember_account(account: &Account) -> Result<(), String> {
    account.validate()?;
    let directory = session_dir()?;
    let path = directory.join("account.json");
    let staging = directory.join(format!("account-{}.tmp", std::process::id()));
    let mut file = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&staging)
        .map_err(|e| format!("Could not remember the signed-in account: {e}"))?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, account).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(staging, path).map_err(|e| e.to_string())
}
pub fn login_account(region: &str, helper: &Path) -> Result<Account, String> {
    // Account sign-in intentionally does not depend on any game or Wine prefix.
    let bytes = run_login(helper, region, None)?;
    let account: Account = serde_json::from_slice(&bytes).map_err(|_| {
        "Invalid Battle.net account response. Open Details for login diagnostics.".to_owned()
    })?;
    account.validate()?;
    remember_account(&account)?;
    Ok(account)
}
/// A cached identity is not proof that Blizzard still accepts the cookies.
pub fn check_account_session(region: &str, helper: &Path) -> Result<Option<Account>, String> {
    let bytes = match run_login_start(helper, region, None, None, true) {
        Ok(bytes) => bytes,
        Err(error) if error == "SESSION_EXPIRED" => return Ok(None),
        Err(error) => return Err(error),
    };
    let account: Account =
        serde_json::from_slice(&bytes).map_err(|_| "Could not verify the saved session.")?;
    account.validate()?;
    Ok(Some(account))
}
// Tickets are opaque and provide no supported refresh-token endpoint. Bound
// reuse locally; renewal uses the existing browser cookies and official flow.
fn authorization_stamp(game: &Game, account: &Account) -> Result<PathBuf, String> {
    let identity = format!(
        "{}|{}|{}|{}",
        game.prefix,
        crate::catalog::product(&game.product_code)?.auth_code,
        game.region,
        account.account_id
    );
    Ok(session_dir()?.join(format!("authorization-{:x}", md5::compute(identity))))
}
fn authorization_recent(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age < Duration::from_secs(24 * 3600))
}
pub fn authorize_game(game: &Game, account: &Account, helper: &Path) -> Result<(), String> {
    if matches!(game.product_code.as_str(), "W1R" | "W2R") {
        account.validate()?;
        runtime_command(game)?;
        return Ok(());
    }
    crate::catalog::product(&game.product_code)?;
    account.validate()?;
    runtime_command(game)?;
    let mode = format!(
        "probe-account\n{}\n{}\n{}",
        account.account_id, game.region, game.locale
    );
    if authorization_recent(&authorization_stamp(game, account)?)
        && exchange(game, &mode, None).is_ok()
    {
        return Ok(());
    }
    refresh_game_authorization(game, account, helper)
}
pub fn refresh_game_authorization(
    game: &Game,
    account: &Account,
    helper: &Path,
) -> Result<(), String> {
    account.validate()?;
    crate::catalog::product(&game.product_code)?;
    runtime_command(game)?;
    let bytes = run_login(
        helper,
        &game.region,
        Some(crate::catalog::product(&game.product_code)?.auth_code),
    )?;
    let ticket = Ticket::parse(
        std::str::from_utf8(&bytes).map_err(|_| "Invalid game authorization response.")?,
    )?;
    if ticket.account_id() != account.account_id {
        return Err("Game authorization belongs to a different Battle.net account. Sign in again before launching.".into());
    }
    store(game, ticket)?;
    let path = authorization_stamp(game, account)?;
    let mut stamp = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "Authorization saved, but its renewal time could not be recorded.")?;
    stamp.write_all(b"renewed\n").map_err(|e| e.to_string())
}
pub fn sign_out_account(game: &Game) -> Result<(), String> {
    let directory = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")
        .ok_or("Cannot resolve account storage.")?
        .data_local_dir()
        .join("web-session");
    if directory.exists() {
        fs::remove_dir_all(directory)
            .map_err(|e| format!("Could not remove the saved browser session: {e}"))?;
    }
    if runtime_command(game).is_ok() {
        clear(game)?;
    }
    Ok(())
}
pub fn probe(game: &Game) -> Result<(), String> {
    exchange(game, "probe", None)
}
pub fn clear(game: &Game) -> Result<(), String> {
    exchange(game, "clear", None)
}

pub fn install_root(game: &Game) -> Result<PathBuf, String> {
    let product = crate::catalog::product(&game.product_code)?;
    let root = crate::catalog::target_root(game)?;
    let expected = root.join(product.executable);
    if !expected.is_file()
        && crate::discovery::resolve(&root, product.executable).is_none_or(|p| !p.is_file())
    {
        return Err(format!(
            "Locate or install {} in Game settings.",
            product.name
        ));
    }
    Ok(root)
}
pub fn installed_build(game: &Game) -> Option<String> {
    let root = install_root(game).ok()?;
    if game.product_code == "S2" {
        return active_build(&root, &game.region);
    }
    let info = fs::read_to_string(root.join(".build.info")).ok()?;
    let mut lines = info.lines();
    let columns: Vec<_> = lines
        .next()?
        .split('|')
        .map(|s| s.split('!').next().unwrap_or(""))
        .collect();
    let active = columns.iter().position(|s| *s == "Active")?;
    let version = columns.iter().position(|s| *s == "Version")?;
    let branch = columns.iter().position(|s| *s == "Branch")?;
    let product_column = columns.iter().position(|s| *s == "Product");
    let slug = crate::catalog::product(&game.product_code).ok()?.slug;
    let rows: Vec<Vec<_>> = lines
        .map(|s| s.split('|').collect::<Vec<_>>())
        .filter(|r| {
            r.get(active) == Some(&"1")
                && product_column
                    .and_then(|i| r.get(i))
                    .is_none_or(|p| p.is_empty() || p.eq_ignore_ascii_case(slug))
        })
        .collect();
    let row = rows
        .iter()
        .find(|r| {
            r.get(branch)
                .is_some_and(|r| r.eq_ignore_ascii_case(&game.region))
        })
        .or_else(|| (rows.len() == 1).then(|| &rows[0]))?;
    let build = row
        .get(version)?
        .rsplit('.')
        .next()?
        .split(|c: char| !c.is_ascii_digit())
        .next()?;
    if build.is_empty() || !build.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("Base{build}"))
}
pub(crate) fn active_build(root: &Path, region: &str) -> Option<String> {
    let info = fs::read_to_string(root.join(".build.info")).ok()?;
    let mut lines = info.lines();
    let header: Vec<_> = lines.next()?.split('|').collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|value| value.split('!').next() == Some(name))
    };
    let active = column("Active")?;
    let version = column("Version")?;
    let branch = column("Branch")?;
    for line in lines {
        let values: Vec<_> = line.split('|').collect();
        if values.get(active) != Some(&"1") || !values.get(branch)?.eq_ignore_ascii_case(region) {
            continue;
        }
        let number = values.get(version)?.rsplit('.').next()?;
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let build = format!("Base{number}");
        if root
            .join("Versions")
            .join(&build)
            .join("SC2_x64.exe")
            .is_file()
        {
            return Some(build);
        }
    }
    None
}
pub fn game_running(game: &Game) -> bool {
    let Ok(root) = install_root(game) else {
        return false;
    };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let support = if matches!(game.product_code.as_str(), "S2" | "Hero") {
        root.join("Support64")
    } else {
        root.join(
            crate::catalog::product(&game.product_code)
                .map(|p| p.game_dir)
                .unwrap_or(""),
        )
    };
    let Ok(processes) = fs::read_dir("/proc") else {
        return false;
    };
    processes.flatten().any(|process| {
        if !process
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            return false;
        }
        let path = process.path();
        let Ok(file) = File::open(path.join("cmdline")) else {
            return false;
        };
        let mut bytes = Vec::new();
        if file.take(4096).read_to_end(&mut bytes).is_err() {
            return false;
        }
        let first = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
        let name = String::from_utf8_lossy(first).replace('\\', "/");
        name.rsplit('/').next().is_some_and(|name| {
            name.eq_ignore_ascii_case(
                crate::catalog::product(&game.product_code)
                    .map(|p| p.process)
                    .unwrap_or(""),
            )
        }) && fs::read_link(path.join("cwd"))
            .is_ok_and(|cwd| cwd == support || cwd.starts_with(&root))
    })
}
pub fn launch(game: &Game) -> Result<Child, String> {
    crate::patcher::recover_installation(game)?;
    if game_running(game) {
        return Err(format!("{} is already running.", game.name));
    }
    if !matches!(game.product_code.as_str(), "W1R" | "W2R") {
        probe(game)?;
    }
    let root = install_root(game)?;
    let windows_root = format!("Z:{}", root.to_string_lossy().replace('/', "\\"));
    // SC2's switcher supplies startup setup required by current builds.
    // The bridge follows its SC2 child, so switcher exit 0 is not game closure.
    let payload = format!("{}\nresume\n{windows_root}\n\n", bridge_header(game)?);
    let (mut child, network) = spawn_bridge(game, &payload, "last-launch.log")?;
    if let Some(stream) = network {
        use std::io::BufRead;
        let mut response = String::new();
        let result = std::io::BufReader::new(stream.take(64)).read_line(&mut response);
        if result.is_ok() && response == "started\n" {
            return Ok(child);
        }
        let _ = child.kill();
        let _ = child.wait();
        return Err(
            "The game launcher did not start the game. Open Launch details for the runtime log."
                .into(),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(50);
    loop {
        let text = fs::read_to_string(game_log_path(game)).unwrap_or_default();
        if text.starts_with("started\n") {
            return Ok(child);
        }
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err(
                "The game launcher could not start the game. Open Launch details for the runtime log."
                    .into(),
            );
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Game startup timed out.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub fn record_game_exit(game: &Game, status: std::process::ExitStatus) {
    if let Ok(mut file) = File::options().append(true).open(game_log_path(game)) {
        let _ = writeln!(file, "Game runtime finished: {status}");
    }
}
pub fn log_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "battlenet-linux-launcher")
        .unwrap()
        .cache_dir()
        .join("last-launch.log")
}
pub fn game_log_path(game: &Game) -> PathBuf {
    if game.product_code == "S2" {
        return log_path();
    }
    let slug = crate::catalog::product(&game.product_code)
        .map(|p| p.slug)
        .unwrap_or("unknown");
    log_path()
        .parent()
        .unwrap()
        .join(slug)
        .join("last-launch.log")
}
pub fn read_log(game: &Game) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut result = String::new();
    for name in ["last-launch.log", "runtime.log", "login-status.log"] {
        let path = if name == "login-status.log" {
            log_path().with_file_name(name)
        } else {
            game_log_path(game).with_file_name(name)
        };
        if let Ok(mut file) = File::open(&path) {
            let size = file.metadata()?.len();
            file.seek(SeekFrom::Start(size.saturating_sub(16384)))?;
            let mut bytes = Vec::new();
            file.take(16384).read_to_end(&mut bytes)?;
            result.push_str(&format!("{name}\n"));
            result.push_str(&String::from_utf8_lossy(&bytes));
            result.push('\n');
        }
    }
    if result.is_empty() {
        result = "No launch diagnostics yet.".into();
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_editor_snap_schema_paths_without_removing_system_or_flatpak_paths() {
        assert_eq!(
            system_data_dirs(
                "/home/test/snap/code/271/.local/share:/snap/code/271/usr/share:/var/lib/flatpak/exports/share:/usr/share/"
            ),
            "/var/lib/flatpak/exports/share:/usr/share/"
        );
        assert_eq!(
            system_data_dirs("/snap/code/271/usr/share"),
            "/usr/local/share:/usr/share"
        );
    }
    #[test]
    fn distinguishes_login_crashes_and_network_failures_from_cancellation() {
        assert!(login_error(None, Some(6), &[]).contains("crashed"));
        assert!(login_error(Some(3), None, &[]).contains("desktop"));
        assert!(
            login_error(Some(1), None, &["network_failed", "window_closed"]).contains("connection")
        );
        assert_eq!(
            login_error(Some(1), None, &["cancelled"]),
            "Sign-in cancelled."
        );
        assert!(!login_error(Some(1), None, &["window_closed"]).contains("cancelled"));
    }
    #[test]
    fn resolves_only_the_active_regional_build_from_install_metadata() {
        let root = std::env::temp_dir().join(format!("bnet-build-test-{}", std::process::id()));
        fs::create_dir_all(root.join("Versions/Base12345")).unwrap();
        fs::write(root.join("Versions/Base12345/SC2_x64.exe"), "").unwrap();
        fs::write(
            root.join(".build.info"),
            "Branch!STRING:0|Active!DEC:1|Version!STRING:0\neu|0|5.0.1.99999\neu|1|5.0.1.12345\n",
        )
        .unwrap();
        assert_eq!(active_build(&root, "EU"), Some("Base12345".into()));
        assert_eq!(active_build(&root, "US"), None);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn proton_uses_umu_and_preserves_the_selected_prefix() {
        let root = std::env::temp_dir().join(format!("bnet-runtime-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("system.reg"), "").unwrap();
        fs::write(root.join("proton"), "").unwrap();
        let game = Game {
            prefix: root.to_string_lossy().into(),
            proton: root.to_string_lossy().into(),
            umu: "/bin/true".into(),
            ..Game::default()
        };
        let cmd = runtime_command(&game).unwrap();
        assert_eq!(cmd.get_program(), "/bin/true");
        assert!(
            cmd.get_envs()
                .any(|(key, value)| key == "PROTON_VERB"
                    && value == Some(std::ffi::OsStr::new("run")))
        );
        let values: std::collections::HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(
            values.get(std::ffi::OsStr::new("WINEPREFIX")),
            Some(&Some(root.as_os_str()))
        );
        assert_eq!(
            values.get(std::ffi::OsStr::new("PROTONPATH")),
            Some(&Some(root.as_os_str()))
        );
        let wine_game = Game {
            proton: String::new(),
            runtime: "/bin/echo".into(),
            ..game
        };
        assert_eq!(
            runtime_command(&wine_game).unwrap().get_program(),
            "/bin/echo"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_unconfigured_install() {
        assert!(install_root(&Game::default()).is_err());
        assert!(runtime_command(&Game::default()).is_err());
    }
}

/// Open links through the desktop host, including from a Flatpak installation.
pub fn open_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "Invalid website address.")?;
    if parsed.scheme() != "https" {
        return Err("Only secure website links are supported.".into());
    }
    let output = host_command("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not open your browser: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err("Could not open the default browser. Check your desktop's browser association.".into())
    }
}

pub fn chat_ticket(region: &str, account: &Account, start: &str) -> Result<Ticket, String> {
    account.validate()?;
    let helper = login_helper().ok_or("Battle.net sign-in component is missing.")?;
    if start.len() > 8192 {
        return Err("Invalid chat challenge".into());
    }
    let bytes = run_login_start(&helper, region, Some("app"), Some(start), false)?;
    let ticket =
        Ticket::parse(std::str::from_utf8(&bytes).map_err(|_| "Invalid chat authorization.")?)?;
    if ticket.account_id() != account.account_id {
        return Err("Chat authorization belongs to a different account.".into());
    }
    Ok(ticket)
}

pub fn uses_proton(game: &Game) -> bool {
    game.runtime_kind == "proton" || (game.runtime_kind == "auto" && !game.proton.is_empty())
}

#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn ticket_reuse_requires_a_recent_non_future_renewal() {
        let path = std::env::temp_dir().join(format!("bnet-renewal-test-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        assert!(!authorization_recent(&path));
        let file = File::create(&path).unwrap();
        assert!(authorization_recent(&path));
        let now = std::time::SystemTime::now();
        file.set_times(fs::FileTimes::new().set_modified(now - Duration::from_secs(86401)))
            .unwrap();
        assert!(!authorization_recent(&path));
        file.set_times(fs::FileTimes::new().set_modified(now + Duration::from_secs(3600)))
            .unwrap();
        assert!(!authorization_recent(&path));
        fs::remove_file(path).unwrap();
    }
}
