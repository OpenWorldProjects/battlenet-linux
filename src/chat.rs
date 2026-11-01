//! Native Battle.net v2 RPC transport. Wire fields follow public protocol
//! descriptions; no foreign implementation code is included.
use crate::{auth::Account, runner};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    pub high: u64,
    pub low: u64,
}
#[derive(Clone)]
pub struct Friend {
    pub id: Entity,
    pub name: String,
}
#[derive(Clone)]
pub struct Message {
    pub peer: Entity,
    pub name: String,
    pub text: String,
    pub outgoing: bool,
}
pub enum Event {
    Avatar(Entity, u32),
    Status(String),
    Connected,
    AccountVerified(Account),
    SessionRenewed,
    FriendsLoaded,
    Authorizing,
    Friend(Friend),
    Removed(Entity),
    Received(Message),
    Sent(Message),
    Failed(String),
    Disconnected,
}
pub enum Command {
    Send(Friend, String),
    Disconnect,
}
pub struct Connection {
    pub commands: Sender<Command>,
    pub events: Receiver<Event>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}
impl Connection {
    pub fn start(region: String, account: Account, ctx: eframe::egui::Context) -> Self {
        Self::start_account(region, Some(account), ctx)
    }
    pub fn login(region: String, ctx: eframe::egui::Context) -> Self {
        Self::start_account(region, None, ctx)
    }
    fn start_account(region: String, account: Option<Account>, ctx: eframe::egui::Context) -> Self {
        let (commands, rx) = mpsc::channel();
        let (tx, events) = mpsc::channel();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        std::thread::spawn(move || {
            let emit = |event| {
                if matches!(&event, Event::Received(_)) {
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::RequestUserAttention(
                        eframe::egui::UserAttentionType::Informational,
                    ));
                    let _ = runner::host_command("notify-send")
                        .args(["Battle.net", "You have a new chat message."])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
                let _ = tx.send(event);
                ctx.request_repaint();
            };
            if let Err(error) = run(&region, account.as_ref(), rx, &emit, &worker_cancelled) {
                emit(Event::Failed(error));
            }
            worker_cancelled.store(true, std::sync::atomic::Ordering::Release);
            emit(Event::Disconnected);
        });
        Self {
            commands,
            events,
            cancelled,
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = self.commands.send(Command::Disconnect);
    }
}
#[derive(Default)]
pub struct State {
    pub connection: Option<Connection>,
    pub connected: bool,
    pub friends_loaded: bool,
    pub authorizing: bool,
    pub verified_account: Option<Account>,
    pub last_renewed: Option<Instant>,
    pub error: bool,
    pub status: String,
    pub contacts: Vec<Friend>,
    pub avatars: HashMap<(u64, u64), u32>,
    pub messages: Vec<Message>,
    pub selected: Option<Entity>,
    pub draft: String,
    pub open: bool,
    pub unread: usize,
}
impl State {
    pub fn disconnect(&mut self) {
        if let Some(c) = self.connection.take() {
            let _ = c.commands.send(Command::Disconnect);
        }
        self.connected = false;
        self.friends_loaded = false;
        self.authorizing = false;
        self.verified_account = None;
        self.last_renewed = None;
        self.error = false;
        self.contacts.clear();
        self.avatars.clear();
        self.messages.clear();
        self.selected = None;
        self.draft.clear();
        self.unread = 0;
        self.status = "Disconnected".into();
    }
    pub fn poll(&mut self) -> bool {
        let events: Vec<_> = self
            .connection
            .as_ref()
            .map(|c| c.events.try_iter().take(128).collect())
            .unwrap_or_default();
        let needs_another_frame = events.len() == 128;
        for event in events {
            match event {
                Event::Avatar(id, avatar) => {
                    self.avatars.insert((id.high, id.low), avatar);
                }
                Event::Status(s) => {
                    self.status = s;
                    self.error = false;
                }
                Event::AccountVerified(account) => self.verified_account = Some(account),
                Event::FriendsLoaded => self.friends_loaded = true,
                Event::Authorizing => self.authorizing = true,
                Event::SessionRenewed => self.last_renewed = Some(Instant::now()),
                Event::Connected => {
                    self.friends_loaded = false;
                    self.authorizing = false;
                    self.connected = true;
                    self.error = false;
                    self.status = "Connected to Battle.net chat".into();
                }
                Event::Friend(friend) => {
                    if let Some(old) = self.contacts.iter_mut().find(|f| f.id == friend.id) {
                        if friend.name != "Battle.net contact" || old.name == "Battle.net contact" {
                            *old = friend;
                        }
                    } else if self.contacts.len() < 5000 {
                        self.contacts.push(friend);
                    }
                    self.contacts.sort_by_key(|f| f.name.to_lowercase());
                }
                Event::Removed(id) => self.contacts.retain(|f| f.id != id),
                Event::Received(message) => {
                    if !self.contacts.iter().any(|f| f.id == message.peer) {
                        self.contacts.push(Friend {
                            id: message.peer,
                            name: message.name.clone(),
                        });
                    }
                    if self.open && self.selected != Some(message.peer) {
                        self.status = format!("New message from {}", message.name);
                    }
                    if !self.open {
                        self.unread = self.unread.saturating_add(1);
                    }
                    self.messages.push(message);
                }
                Event::Sent(message) => {
                    self.messages.push(message);
                    self.status = "Message accepted by Battle.net".into();
                }
                Event::Failed(error) => {
                    self.status = error;
                    self.error = true;
                }
                Event::Disconnected => {
                    self.friends_loaded = false;
                    self.authorizing = false;
                    self.connected = false;
                    self.connection = None;
                }
            }
        }
        if self.messages.len() > 1000 {
            self.messages.drain(..self.messages.len() - 1000);
        }
        needs_another_frame
    }
}
fn varint(mut n: u64, out: &mut Vec<u8>) {
    while n >= 128 {
        out.push((n as u8) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn number(n: u32, value: u64, out: &mut Vec<u8>) {
    varint((n as u64) << 3, out);
    varint(value, out);
}
fn bytes(n: u32, value: &[u8], out: &mut Vec<u8>) {
    varint(((n as u64) << 3) | 2, out);
    varint(value.len() as u64, out);
    out.extend_from_slice(value);
}
fn fixed(n: u32, value: u64, width: usize, out: &mut Vec<u8>) {
    varint(((n as u64) << 3) | if width == 8 { 1 } else { 5 }, out);
    out.extend_from_slice(&value.to_le_bytes()[..width]);
}
fn read_varint(input: &[u8], at: &mut usize) -> Result<u64> {
    let mut n = 0;
    for shift in (0..70).step_by(7) {
        let b = *input.get(*at).ok_or("Truncated chat response")?;
        *at += 1;
        if shift == 63 && b > 1 {
            return Err("Invalid chat integer".into());
        }
        n |= ((b & 127) as u64) << shift;
        if b & 128 == 0 {
            return Ok(n);
        }
    }
    Err("Invalid chat integer".into())
}
struct Field<'a> {
    id: u32,
    value: u64,
    data: &'a [u8],
}
fn fields(input: &[u8]) -> Result<Vec<Field<'_>>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < input.len() {
        if out.len() > 20000 {
            return Err("Oversized chat response".into());
        }
        let tag = read_varint(input, &mut at)?;
        let id = (tag >> 3) as u32;
        if id == 0 {
            return Err("Invalid chat field".into());
        }
        let mut value = 0;
        let mut data = &[][..];
        match tag & 7 {
            0 => value = read_varint(input, &mut at)?,
            1 | 5 => {
                let size = if tag & 7 == 1 { 8 } else { 4 };
                let end = at.checked_add(size).ok_or("Invalid chat length")?;
                let raw = input.get(at..end).ok_or("Truncated chat value")?;
                let mut buf = [0; 8];
                buf[..size].copy_from_slice(raw);
                value = u64::from_le_bytes(buf);
                at = end;
            }
            2 => {
                let size = usize::try_from(read_varint(input, &mut at)?)
                    .map_err(|_| "Invalid chat length")?;
                let end = at.checked_add(size).ok_or("Invalid chat length")?;
                data = input.get(at..end).ok_or("Truncated chat content")?;
                at = end;
            }
            _ => return Err("Unsupported chat wire field".into()),
        }
        out.push(Field { id, value, data });
    }
    Ok(out)
}
fn num(data: &[u8], id: u32) -> Result<u64> {
    Ok(fields(data)?
        .iter()
        .find(|f| f.id == id)
        .map(|f| f.value)
        .unwrap_or(0))
}
fn blob(data: &[u8], id: u32) -> Result<&[u8]> {
    Ok(fields(data)?
        .iter()
        .find(|f| f.id == id)
        .map(|f| f.data)
        .unwrap_or(&[]))
}
fn text(data: &[u8], id: u32) -> Result<String> {
    let value = std::str::from_utf8(blob(data, id)?).map_err(|_| "Invalid chat text")?;
    if value.len() > 8192 {
        return Err("Chat text is too large".into());
    }
    Ok(value
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect())
}
fn entity(data: &[u8]) -> Result<Entity> {
    if data.is_empty() {
        return Err("Missing chat identity".into());
    }
    Ok(Entity {
        high: num(data, 1)?,
        low: num(data, 2)?,
    })
}
fn encode_entity(id: Entity) -> Vec<u8> {
    let mut out = Vec::new();
    fixed(1, id.high, 8, &mut out);
    fixed(2, id.low, 8, &mut out);
    out
}
fn friend(data: &[u8]) -> Result<Friend> {
    let id = entity(blob(data, 1)?)?;
    let name = text(data, 7)?;
    Ok(Friend {
        id,
        name: if name.is_empty() {
            "Battle.net contact".into()
        } else {
            name
        },
    })
}
fn hash(name: &str) -> u32 {
    name.bytes().fold(2166136261u32, |h, b| {
        (h ^ (b as u32)).wrapping_mul(16777619)
    })
}
const IMPORTS: [&str; 4] = [
    "bnet.protocol.authentication.AuthenticationServer",
    "bnet.protocol.friends.FriendsService",
    "bnet.protocol.notification.NotificationService",
    "bnet.protocol.presence.PresenceService",
];
const EXPORTS: [&str; 5] = [
    "bnet.protocol.authentication.AuthenticationClient",
    "bnet.protocol.challenge.ChallengeNotify",
    "bnet.protocol.friends.FriendsNotify",
    "bnet.protocol.connection.ConnectionService",
    "bnet.protocol.notification.NotificationListener",
];
enum Pending {
    Connect,
    Auth,
    Friends,
    Renewal,
    Presence(Entity),
    Send(Message),
}
struct Wire {
    stream: rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
    seq: u32,
    bindless: bool,
    pending: HashMap<u32, Pending>,
}
impl Wire {
    fn packet(&mut self, service: u32, method: u32, token: u32, body: &[u8]) -> Result<()> {
        let mut header = Vec::new();
        if self.bindless && service != 254 {
            number(1, 0, &mut header);
            fixed(
                11,
                if service == 0 {
                    hash("bnet.protocol.connection.ConnectionService")
                } else {
                    service
                } as u64,
                4,
                &mut header,
            );
        } else {
            number(1, service as u64, &mut header);
        }
        if service == 254 {
            number(9, 1, &mut header);
        }
        number(2, method as u64, &mut header);
        number(3, token as u64, &mut header);
        number(5, body.len() as u64, &mut header);
        self.stream
            .write_all(&(header.len() as u16).to_be_bytes())
            .and_then(|_| self.stream.write_all(&header))
            .and_then(|_| self.stream.write_all(body))
            .and_then(|_| self.stream.flush())
            .map_err(|_| "Battle.net chat connection was interrupted".into())
    }
    fn request(
        &mut self,
        service: u32,
        method: u32,
        body: &[u8],
        pending: Option<Pending>,
    ) -> Result<()> {
        self.seq = self.seq.wrapping_add(1);
        let token = self.seq;
        if self.pending.len() >= 128 {
            return Err("Too many pending chat requests".into());
        }
        if let Some(p) = pending {
            self.pending.insert(token, p);
        }
        self.packet(service, method, token, body)
    }
}
fn verified_profile(id: Entity, expected: Option<&Account>) -> Result<Account> {
    if id.low == 0 {
        return Err("Chat did not return an account identity".into());
    }
    let account_id = id.low.to_string();
    if expected.is_some_and(|account| account.account_id != account_id) {
        return Err("Chat returned a different Battle.net account".into());
    }
    let account = Account {
        account_id,
        battle_tag: expected.and_then(|account| account.battle_tag.clone()),
    };
    account.validate()?;
    Ok(account)
}

fn run(
    region: &str,
    account: Option<&Account>,
    commands: Receiver<Command>,
    emit: &impl Fn(Event),
    cancelled: &Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let result = run_connection(region, account, &commands, emit, true, cancelled);
    if matches!(&result,Err(error) if error=="CACHED_SESSION_REJECTED") {
        if let Some(account) = account {
            crate::session::clear(region, account);
        }
        return run_connection(region, account, &commands, emit, false, cancelled);
    }
    result
}
fn run_connection(
    region: &str,
    account: Option<&Account>,
    commands: &Receiver<Command>,
    emit: &impl Fn(Event),
    use_cached: bool,
    cancelled: &Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    if let Some(account) = account {
        account.validate()?;
    }
    let prefix = match region {
        "EU" => "eu",
        "KR" => "kr",
        "US" => "us",
        _ => return Err("Chat currently supports EU, US, and KR accounts".into()),
    };
    let host = format!("{prefix}.actual.battle.net");
    emit(Event::Status("Connecting to Battle.net chat…".into()));
    let addresses = (host.as_str(), 1119)
        .to_socket_addrs()
        .map_err(|_| "Could not resolve Battle.net chat")?;
    let mut socket = None;
    for addr in addresses.take(8) {
        if let Ok(s) = TcpStream::connect_timeout(&addr, Duration::from_secs(8)) {
            socket = Some(s);
            break;
        }
    }
    let socket = socket.ok_or("Battle.net chat server could not be reached")?;
    socket
        .set_read_timeout(Some(Duration::from_secs(8)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(8)))
        .map_err(|e| e.to_string())?;
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host).map_err(|_| "Invalid chat host")?;
    let conn = rustls::ClientConnection::new(Arc::new(config), name)
        .map_err(|_| "Could not initialize secure chat")?;
    let mut wire = Wire {
        stream: rustls::StreamOwned::new(conn, socket),
        seq: 0,
        bindless: false,
        pending: HashMap::new(),
    };
    let mut bind = Vec::new();
    for name in IMPORTS {
        fixed(1, hash(name) as u64, 4, &mut bind);
    }
    for (i, name) in EXPORTS.iter().enumerate() {
        let mut service = Vec::new();
        fixed(1, hash(name) as u64, 4, &mut service);
        number(2, (i + 1) as u64, &mut service);
        bytes(2, &service, &mut bind);
    }
    let mut connect = Vec::new();
    bytes(2, &bind, &mut connect);
    number(3, 1, &mut connect);
    wire.request(0, 1, &connect, Some(Pending::Connect))?;
    wire.stream
        .sock
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|e| e.to_string())?;
    let mut services = [0u32; 4];
    let mut input = Vec::new();
    let cached = if use_cached {
        account.and_then(|a| crate::session::load(region, a))
    } else {
        None
    };
    let mut cached_attempt = cached.is_some();
    let mut next_renewal = Instant::now();
    let mut renewal_started: Option<Instant> = None;
    let mut connected = false;
    let mut own_account: Option<(Entity, Account)> = None;
    let mut auth: Option<Receiver<Result<crate::auth::Ticket>>> = None;
    let mut auth_started = false;
    let mut deadline = Instant::now() + Duration::from_secs(45);
    let mut last_ping = Instant::now();
    let mut names = VecDeque::new();
    let mut portraits = Vec::new();
    let mut next_profile_refresh = Instant::now() + Duration::from_secs(300);
    loop {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        match commands.try_recv() {
            Ok(Command::Disconnect) | Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Ok(Command::Send(to, body)) => {
                if !connected {
                    return Err("Chat is not connected".into());
                }
                if body.trim().is_empty() || body.len() > 4096 {
                    return Err("Messages must contain 1–4096 bytes".into());
                }
                let mut notification = Vec::new();
                bytes(2, &encode_entity(to.id), &mut notification);
                bytes(3, b"WHISPER", &mut notification);
                let mut value = Vec::new();
                bytes(5, body.as_bytes(), &mut value);
                let mut attr = Vec::new();
                bytes(1, b"whisper", &mut attr);
                bytes(2, &value, &mut attr);
                bytes(4, &attr, &mut notification);
                wire.request(
                    services[2],
                    1,
                    &notification,
                    Some(Pending::Send(Message {
                        peer: to.id,
                        name: to.name,
                        text: body,
                        outgoing: true,
                    })),
                )?;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if connected && Instant::now() >= next_profile_refresh {
            for id in &portraits {
                if !names.contains(id) {
                    names.push_back(*id);
                }
            }
            next_profile_refresh = Instant::now() + Duration::from_secs(300);
        }
        for _ in 0..8 {
            if wire.pending.len() >= 32 {
                break;
            }
            let Some(id) = names.pop_front() else {
                break;
            };
            if portraits.len() < 5001 && !portraits.contains(&id) {
                portraits.push(id);
            }
            let mut query = Vec::new();
            bytes(1, &encode_entity(id), &mut query);
            let mut key = Vec::new();
            number(1, 0x424e, &mut key);
            number(2, 1, &mut key);
            number(3, 4, &mut key);
            bytes(2, &key, &mut query);
            let mut avatar_key = Vec::new();
            number(1, 0x424e, &mut avatar_key);
            number(2, 1, &mut avatar_key);
            number(3, 13, &mut avatar_key);
            bytes(2, &avatar_key, &mut query);
            wire.request(services[3], 4, &query, Some(Pending::Presence(id)))?;
        }
        if let Some(rx) = &auth {
            if let Ok(ticket) = rx.try_recv() {
                let ticket = ticket?;
                let mut body = Zeroizing::new(Vec::new());
                bytes(1, ticket.expose().as_bytes(), &mut body);
                wire.request(services[0], 7, &body, Some(Pending::Auth))?;
                auth = None;
            }
        }
        if !connected && Instant::now() > deadline {
            return Err("Chat authorization timed out. Reconnect to try again.".into());
        }
        if renewal_started.is_some_and(|at| at.elapsed() > Duration::from_secs(30)) {
            wire.pending
                .retain(|_, pending| !matches!(pending, Pending::Renewal));
            renewal_started = None;
            next_renewal = Instant::now() + Duration::from_secs(300);
            emit(Event::Status(
                "Session refresh timed out; it will retry automatically.".into(),
            ));
        }
        if connected
            && Instant::now() >= next_renewal
            && !wire.pending.values().any(|p| matches!(p, Pending::Renewal))
        {
            wire.request(services[0], 8, &[], Some(Pending::Renewal))?;
            renewal_started = Some(Instant::now());
            next_renewal = Instant::now() + Duration::from_secs(30 * 60);
        }
        if connected && last_ping.elapsed() > Duration::from_secs(25) {
            wire.request(0, 5, &[], None)?;
            last_ping = Instant::now();
        }
        let mut chunk = [0u8; 8192];
        match wire.stream.read(&mut chunk) {
            Ok(0) => return Err("Battle.net closed the chat connection".into()),
            Ok(n) => input.extend_from_slice(&chunk[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => {
                return Err(
                    "Secure chat connection failed; certificate verification remains enabled"
                        .into(),
                );
            }
        }
        if input.len() > 2 * 1024 * 1024 {
            return Err("Oversized chat packet".into());
        }
        loop {
            if input.len() < 2 {
                break;
            }
            let hlen = u16::from_be_bytes([input[0], input[1]]) as usize;
            if hlen == 0 || hlen > 4096 {
                return Err("Invalid chat header".into());
            }
            if input.len() < 2 + hlen {
                break;
            }
            let header = &input[2..2 + hlen];
            let len = num(header, 5)? as usize;
            if len > 1024 * 1024 {
                return Err("Oversized chat message".into());
            }
            if input.len() < 2 + hlen + len {
                break;
            }
            let mut service = num(header, 1)? as u32;
            if wire.bindless && service != 254 {
                let h = num(header, 11)? as u32;
                service = EXPORTS
                    .iter()
                    .position(|name| hash(name) == h)
                    .map(|i| i as u32 + 1)
                    .unwrap_or(service);
            }
            let response_flag = num(header, 9)? != 0;
            let method = num(header, 2)? as u32;
            let token = num(header, 3)? as u32;
            let status = num(header, 6)?;
            let body = Zeroizing::new(input[2 + hlen..2 + hlen + len].to_vec());
            input.drain(..2 + hlen + len);
            if service == 254 || response_flag {
                let pending = wire.pending.remove(&token);
                if status != 0 {
                    if matches!(pending, Some(Pending::Renewal)) {
                        renewal_started = None;
                        emit(Event::Status(format!(
                            "Connected; session renewal was rejected (code {status})."
                        )));
                        continue;
                    }
                    if matches!(pending, Some(Pending::Auth))
                        && cached_attempt
                        && !auth_started
                        && !connected
                    {
                        return Err("CACHED_SESSION_REJECTED".into());
                    }
                    if matches!(pending, Some(Pending::Presence(_))) {
                        continue;
                    }
                    if matches!(pending, Some(Pending::Send(_))) {
                        emit(Event::Failed(format!(
                            "Battle.net rejected the message (code {status})"
                        )));
                        continue;
                    }
                    return Err(format!(
                        "Battle.net rejected the chat request (code {status})"
                    ));
                }
                match pending {
                    Some(Pending::Connect) => {
                        let response = blob(&body, 4)?;
                        let list = blob(response, 1)?;
                        if list.is_empty() {
                            wire.bindless = true;
                            services = IMPORTS.map(hash);
                        } else {
                            let mut at = 0;
                            for id in &mut services {
                                *id = read_varint(list, &mut at)? as u32;
                                if *id == 0 {
                                    return Err("Battle.net did not bind its chat services".into());
                                }
                            }
                        }
                        let mut login = Zeroizing::new(Vec::new());
                        bytes(1, b"App", &mut login);
                        bytes(2, b"Win", &mut login);
                        bytes(3, b"enUS", &mut login);
                        number(6, 1, &mut login);
                        number(10, 1, &mut login);
                        number(11, 1, &mut login);
                        if let Some(credential) = &cached {
                            bytes(12, credential, &mut login);
                        }
                        wire.request(services[0], 1, &login, Some(Pending::Auth))?;
                        emit(Event::Status(
                            "Waiting for Battle.net chat authorization…".into(),
                        ));
                    }
                    Some(Pending::Renewal) => {
                        renewal_started = None;
                        let credential = Zeroizing::new(blob(&body, 1)?.to_vec());
                        if let Some((_, profile)) = &own_account {
                            match crate::session::save(region, profile, &credential) {
                                Ok(()) => emit(Event::SessionRenewed),
                                Err(error) => emit(Event::Status(error)),
                            }
                        }
                    }
                    Some(Pending::Friends) => {
                        for entry in fields(&body)?.iter().filter(|f| f.id == 5) {
                            if let Ok(f) = friend(entry.data) {
                                let id = f.id;
                                emit(Event::Friend(f));
                                if names.len() < 5000 && !names.contains(&id) {
                                    names.push_back(id);
                                }
                            }
                        }
                        emit(Event::FriendsLoaded);
                    }
                    Some(Pending::Presence(id)) => {
                        for field in fields(&body)?.iter().filter(|f| f.id == 2) {
                            if let Some(avatar) = presence_avatar(field.data) {
                                emit(Event::Avatar(id, avatar));
                            }
                            let key = blob(field.data, 1)?;
                            if num(key, 1)? == 0x424e && num(key, 2)? == 1 && num(key, 3)? == 4 {
                                let name = text(blob(field.data, 2)?, 5)?;
                                if !name.is_empty() {
                                    if let Some((_, profile)) =
                                        own_account.as_mut().filter(|(own_id, _)| *own_id == id)
                                    {
                                        profile.battle_tag = Some(name);
                                        emit(Event::AccountVerified(profile.clone()));
                                    } else {
                                        emit(Event::Friend(Friend { id, name }));
                                    }
                                }
                            }
                        }
                    }
                    Some(Pending::Send(message)) => emit(Event::Sent(message)),
                    _ => {}
                }
            } else {
                wire.packet(254, 0, token, &[])?;
                match (service, method) {
                    (1, 5) => {
                        let error = num(&body, 1)?;
                        if error != 0 {
                            if cached_attempt && !auth_started && !connected {
                                return Err("CACHED_SESSION_REJECTED".into());
                            }
                            return Err(format!("Chat login was rejected (code {error})"));
                        }
                        let id = entity(blob(&body, 2)?)?;
                        let mut verified = verified_profile(id, account)?;
                        let tag = text(&body, 7)?;
                        if !tag.is_empty() {
                            verified.battle_tag = Some(tag);
                            verified.validate()?;
                        }
                        own_account = Some((id, verified.clone()));
                        emit(Event::AccountVerified(verified));
                        names.push_front(id);
                        connected = true;
                        emit(Event::Connected);
                        let mut request = Vec::new();
                        number(2, 1, &mut request);
                        wire.request(services[1], 1, &request, Some(Pending::Friends))?;
                    }
                    (2, 3) => {
                        if text(&body, 2)? != "web_auth_url" {
                            return Err(
                                "Battle.net requested an unsupported chat authorization challenge"
                                    .into(),
                            );
                        }
                        if !auth_started {
                            cached_attempt = false;
                            if let Some(account) = account {
                                crate::session::clear(region, account);
                            }
                            auth_started = true;
                            emit(Event::Authorizing);
                            deadline = Instant::now() + Duration::from_secs(300);
                            emit(Event::Status(
                                "Connecting with your saved Battle.net session…".into(),
                            ));
                            let (tx, rx) = mpsc::channel();
                            let challenge = Zeroizing::new(
                                std::str::from_utf8(blob(&body, 3)?)
                                    .map_err(|_| "Invalid chat challenge")?
                                    .to_owned(),
                            );
                            let account = account.cloned();
                            let cancelled = cancelled.clone();
                            let region = region.to_owned();
                            std::thread::spawn(move || {
                                let _ = tx.send(runner::chat_ticket(
                                    &region,
                                    account.as_ref(),
                                    &challenge,
                                    &cancelled,
                                ));
                            });
                            auth = Some(rx);
                        }
                    }
                    (3, 1) | (3, 4) => {
                        if let Ok(f) = friend(blob(&body, 1)?) {
                            if names.len() < 5000 && !names.contains(&f.id) {
                                names.push_back(f.id);
                            }
                            emit(Event::Friend(f));
                        }
                    }
                    (3, 2) => {
                        if let Ok(f) = friend(blob(&body, 1)?) {
                            portraits.retain(|id| *id != f.id);
                            emit(Event::Removed(f.id));
                        }
                    }
                    (4, 3) => {}
                    (4, 4) => {
                        return Err(
                            "Battle.net disconnected chat; another client may have signed in"
                                .into(),
                        );
                    }
                    (5, 1) => {
                        if text(&body, 3)? == "WHISPER" {
                            let sender = blob(&body, 5)?;
                            let peer = entity(if sender.is_empty() {
                                blob(&body, 1)?
                            } else {
                                sender
                            })?;
                            let mut message = None;
                            for attr in fields(&body)?.iter().filter(|f| f.id == 4) {
                                if text(attr.data, 1)? == "whisper" {
                                    message = Some(text(blob(attr.data, 2)?, 5)?);
                                }
                            }
                            if let Some(text) = message {
                                emit(Event::Received(Message {
                                    peer,
                                    name: text_name(&body)?,
                                    text,
                                    outgoing: false,
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
fn text_name(body: &[u8]) -> Result<String> {
    let s = text(body, 7)?;
    Ok(if s.is_empty() {
        "Battle.net contact".into()
    } else {
        s
    })
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    #[test]
    fn initial_login_accepts_only_a_nonzero_server_account() {
        assert!(verified_profile(Entity { high: 1, low: 0 }, None).is_err());
        let profile = verified_profile(
            Entity {
                high: 1,
                low: 12345,
            },
            None,
        )
        .unwrap();
        assert_eq!(profile.account_id, "12345");
        assert!(profile.battle_tag.is_none());
    }
    #[test]
    fn reconnection_cannot_switch_the_remembered_account() {
        let expected = Account {
            account_id: "12345".into(),
            battle_tag: Some("Example#1234".into()),
        };
        assert!(
            verified_profile(
                Entity {
                    high: 1,
                    low: 98765
                },
                Some(&expected)
            )
            .is_err()
        );
        let profile = verified_profile(
            Entity {
                high: 1,
                low: 12345,
            },
            Some(&expected),
        )
        .unwrap();
        assert_eq!(profile.battle_tag, expected.battle_tag);
    }
}

#[cfg(test)]
mod phase_tests {
    use super::*;
    #[test]
    fn queued_friend_updates_schedule_frames_until_the_final_event() {
        let (tx, events) = mpsc::channel();
        let (commands, _) = mpsc::channel();
        let mut state = State {
            connection: Some(Connection {
                commands,
                events,
                cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }),
            ..State::default()
        };
        for _ in 0..1000 {
            tx.send(Event::Status("Updating contacts".into())).unwrap();
        }
        tx.send(Event::FriendsLoaded).unwrap();
        for _ in 0..50 {
            let more = state.poll();
            if !more {
                assert!(state.friends_loaded);
                return;
            }
        }
        panic!("Queued updates did not finish");
    }
    #[test]
    fn friend_loading_follows_authentication_and_subscription_completion() {
        let (tx, events) = mpsc::channel();
        let (commands, _) = mpsc::channel();
        let mut state = State {
            connection: Some(Connection {
                commands,
                events,
                cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }),
            ..State::default()
        };
        tx.send(Event::Authorizing).unwrap();
        state.poll();
        assert!(state.authorizing);
        assert!(!state.connected);
        assert!(!state.friends_loaded);
        tx.send(Event::Connected).unwrap();
        state.poll();
        assert!(state.connected);
        assert!(!state.authorizing);
        assert!(!state.friends_loaded);
        tx.send(Event::FriendsLoaded).unwrap();
        state.poll();
        assert!(state.friends_loaded);
        tx.send(Event::Disconnected).unwrap();
        state.poll();
        assert!(!state.connected);
        assert!(!state.friends_loaded);
        assert!(!state.authorizing);
    }
}

#[cfg(test)]
mod connection_lifetime_tests {
    use super::*;
    #[test]
    fn replacing_connection_cancels_the_old_authentication() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (commands, received) = mpsc::channel();
        let (_, events) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let connection = Connection {
            commands,
            events,
            cancelled: cancelled.clone(),
        };
        drop(connection);
        assert!(cancelled.load(Ordering::Acquire));
        assert!(matches!(
            received.recv_timeout(Duration::from_millis(100)),
            Ok(Command::Disconnect)
        ));
    }
}

// Account presence: BN / account group 1 / avatar field 13, uint Variant field 9.
fn presence_avatar(field: &[u8]) -> Option<u32> {
    let key = blob(field, 1).ok()?;
    if num(key, 1).ok()? != 0x424e || num(key, 2).ok()? != 1 || num(key, 3).ok()? != 13 {
        return None;
    }
    let variant = fields(blob(field, 2).ok()?).ok()?;
    let value = variant.iter().find(|v| v.id == 9)?.value;
    u32::try_from(value).ok().filter(|v| *v <= 1_000_000)
}
#[cfg(test)]
mod avatar_tests {
    use super::*;
    fn field(group: u64, id: u64, variant: u32, value: u64) -> Vec<u8> {
        let mut key = Vec::new();
        number(1, 0x424e, &mut key);
        number(2, group, &mut key);
        number(3, id, &mut key);
        let mut v = Vec::new();
        number(variant, value, &mut v);
        let mut f = Vec::new();
        bytes(1, &key, &mut f);
        bytes(2, &v, &mut f);
        f
    }
    #[test]
    fn avatar_requires_exact_key_and_unsigned_variant() {
        assert_eq!(presence_avatar(&field(1, 13, 9, 324)), Some(324));
        assert_eq!(presence_avatar(&field(1, 13, 9, 0)), Some(0));
        for f in [
            field(2, 13, 9, 324),
            field(1, 4, 9, 324),
            field(1, 13, 3, 324),
            field(1, 13, 9, u64::MAX),
        ] {
            assert_eq!(presence_avatar(&f), None);
        }
    }
}
