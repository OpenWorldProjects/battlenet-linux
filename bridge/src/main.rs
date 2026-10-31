#![no_std]
#![no_main]
// Win32 declarations keep this Rust helper independent of a Windows SDK/CRT.
use core::{ffi::c_void, panic::PanicInfo};
type Handle = *mut c_void;
#[repr(C)]
struct Blob {
    size: u32,
    bytes: *mut u8,
}
#[repr(C)]
struct Startup {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    xc: u32,
    yc: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    input: Handle,
    output: Handle,
    error: Handle,
}
#[repr(C)]
struct Process {
    process: Handle,
    thread: Handle,
    pid: u32,
    tid: u32,
}
#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    executable: [u16; 260],
}
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn Sleep(milliseconds: u32);
    fn GetStdHandle(id: u32) -> Handle;
    fn ReadFile(h: Handle, p: *mut u8, n: u32, read: *mut u32, overlapped: Handle) -> i32;
    fn WriteFile(h: Handle, p: *const u8, n: u32, written: *mut u32, overlapped: Handle) -> i32;
    fn ExitProcess(code: u32) -> !;
    fn LocalFree(p: Handle) -> Handle;
    fn CreateProcessW(
        app: *const u16,
        cmd: *mut u16,
        pa: Handle,
        ta: Handle,
        inherit: i32,
        flags: u32,
        env: Handle,
        cwd: *const u16,
        startup: *mut Startup,
        process: *mut Process,
    ) -> i32;
    fn CloseHandle(h: Handle) -> i32;
    fn WaitForSingleObject(h: Handle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(h: Handle, code: *mut u32) -> i32;
}
#[link(name = "crypt32", kind = "raw-dylib")]
unsafe extern "system" {
    fn CryptProtectData(
        data: *const Blob,
        description: *const u16,
        entropy: *const Blob,
        reserved: Handle,
        prompt: Handle,
        flags: u32,
        out: *mut Blob,
    ) -> i32;
    fn CryptUnprotectData(
        data: *const Blob,
        description: Handle,
        entropy: *const Blob,
        reserved: Handle,
        prompt: Handle,
        flags: u32,
        out: *mut Blob,
    ) -> i32;
}
#[link(name = "advapi32", kind = "raw-dylib")]
unsafe extern "system" {
    fn RegCreateKeyExW(
        h: Handle,
        path: *const u16,
        reserved: u32,
        class: *mut u16,
        options: u32,
        access: u32,
        security: Handle,
        out: *mut Handle,
        disposition: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        h: Handle,
        name: *const u16,
        reserved: u32,
        kind: u32,
        data: *const u8,
        size: u32,
    ) -> i32;
    fn RegQueryValueExW(
        h: Handle,
        name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        size: *mut u32,
    ) -> i32;
    fn RegDeleteValueW(h: Handle, name: *const u16) -> i32;
    fn RegCloseKey(h: Handle) -> i32;
}
#[link(name = "ws2_32", kind = "raw-dylib")]
unsafe extern "system" {
    fn WSAStartup(version: u16, data: *mut u8) -> i32;
    fn socket(family: i32, kind: i32, protocol: i32) -> usize;
    fn connect(socket: usize, address: *const u8, size: i32) -> i32;
    fn recv(socket: usize, bytes: *mut u8, size: i32, flags: i32) -> i32;
    fn send(socket: usize, bytes: *const u8, size: i32, flags: i32) -> i32;
    fn closesocket(socket: usize) -> i32;
    fn setsockopt(socket: usize, level: i32, option: i32, value: *const u8, size: i32) -> i32;
}
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    fn GetEnvironmentVariableA(name: *const u8, value: *mut u8, size: u32) -> u32;
    fn SetEnvironmentVariableA(name: *const u8, value: *const u8) -> i32;
}
static mut CHANNEL: usize = usize::MAX;
fn network_send(bytes: &[u8]) {
    unsafe {
        let mut sent = 0;
        while sent < bytes.len() {
            let count = send(
                CHANNEL,
                bytes[sent..].as_ptr(),
                (bytes.len() - sent) as i32,
                0,
            );
            if count <= 0 {
                break;
            }
            sent += count as usize;
        }
    }
}
fn network_close() {
    unsafe {
        if CHANNEL != usize::MAX {
            closesocket(CHANNEL);
            CHANNEL = usize::MAX;
        }
    }
}
fn network_init() {
    unsafe {
        let mut port = [0u8; 8];
        let mut nonce = [0u8; 65];
        let n = GetEnvironmentVariableA(c"BNET_BRIDGE_PORT".as_ptr().cast(), port.as_mut_ptr(), 8);
        if n == 0 {
            return;
        }
        if n > 5
            || GetEnvironmentVariableA(c"BNET_BRIDGE_NONCE".as_ptr().cast(), nonce.as_mut_ptr(), 65)
                != 64
        {
            finish(8);
        }
        SetEnvironmentVariableA(c"BNET_BRIDGE_PORT".as_ptr().cast(), core::ptr::null());
        SetEnvironmentVariableA(c"BNET_BRIDGE_NONCE".as_ptr().cast(), core::ptr::null());
        let mut number = 0u32;
        for b in &port[..n as usize] {
            if !b.is_ascii_digit() {
                finish(8);
            }
            number = number * 10 + (b - b'0') as u32;
        }
        if number == 0 || number > 65535 {
            finish(8);
        }
        let mut data = [0u8; 512];
        if WSAStartup(0x202, data.as_mut_ptr()) != 0 {
            finish(8);
        }
        CHANNEL = socket(2, 1, 6);
        if CHANNEL == usize::MAX {
            finish(8);
        }
        let address = [
            2u8,
            0,
            (number >> 8) as u8,
            number as u8,
            127,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        if connect(CHANNEL, address.as_ptr(), 16) != 0 {
            finish(8);
        }
        let timeout = 45000u32;
        setsockopt(CHANNEL, 0xffff, 0x1006, (&timeout as *const u32).cast(), 4);
        setsockopt(CHANNEL, 0xffff, 0x1005, (&timeout as *const u32).cast(), 4);
        network_send(&nonce[..64]);
        for b in &mut nonce {
            core::ptr::write_volatile(b, 0);
        }
    }
}
const NULL: Handle = core::ptr::null_mut();
const ENTROPY: [u8; 16] = [
    0xc8, 0x76, 0xf4, 0xae, 0x4c, 0x95, 0x2e, 0xfe, 0xf2, 0xfa, 0x0f, 0x54, 0x19, 0xc0, 0x9c, 0x43,
];
fn wide<const N: usize>(s: &str) -> [u16; N] {
    let mut out = [0; N];
    for (i, c) in s.encode_utf16().enumerate() {
        if i + 1 >= N {
            finish(2);
        }
        out[i] = c;
    }
    out
}
fn finish(code: u32) -> ! {
    unsafe {
        if CHANNEL != usize::MAX {
            network_send(match code {
                87 => b"exit:87\n",
                2 => b"exit:2\n",
                3 => b"exit:3\n",
                4 => b"exit:4\n",
                5 => b"exit:5\n",
                6 => b"exit:6\n",
                _ => b"exit:9\n",
            });
            network_close();
        }
        ExitProcess(code)
    }
}
fn acknowledge(text: &[u8]) {
    unsafe {
        if CHANNEL != usize::MAX {
            network_send(text);
            return;
        }
        let mut n = 0;
        WriteFile(
            GetStdHandle(-11i32 as u32),
            text.as_ptr(),
            text.len() as u32,
            &mut n,
            NULL,
        );
    }
}
fn valid_token(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 37
        && b.len() <= 128
        && matches!(&b[..2], b"US" | b"EU" | b"KR" | b"CN")
        && b[2] == b'-'
        && b[35] == b'-'
        && b[3..35].iter().all(u8::is_ascii_hexdigit)
        && b[36..].iter().all(u8::is_ascii_digit)
}
#[unsafe(no_mangle)]
pub extern "system" fn mainCRTStartup() -> ! {
    network_init();
    let mut input = [0u8; 2048];
    let mut count = 0;
    unsafe {
        loop {
            let mut read = 0;
            let ok = if CHANNEL != usize::MAX {
                let received = recv(
                    CHANNEL,
                    input[count..].as_mut_ptr(),
                    (input.len() - count) as i32,
                    0,
                );
                if received < 0 {
                    finish(8);
                }
                read = received as u32;
                1
            } else {
                ReadFile(
                    GetStdHandle(-10i32 as u32),
                    input[count..].as_mut_ptr(),
                    (input.len() - count) as u32,
                    &mut read,
                    NULL,
                )
            };
            if ok == 0 || read == 0 {
                break;
            }
            count += read as usize;
            if count == input.len() {
                finish(2);
            }
        }
    }
    let text = core::str::from_utf8(&input[..count]).unwrap_or_else(|_| finish(2));
    if !text.ends_with('\n') {
        finish(2);
    }
    let mut lines = text.split('\n');
    let magic = lines.next().unwrap_or("");
    let product = if magic == "BNET-SC2-1" {
        "S2"
    } else if magic == "BNET-GAME-1" {
        let code = lines.next().unwrap_or("");
        if !matches!(
            code,
            "S2" | "S1"
                | "Hero"
                | "Pro"
                | "W3"
                | "W1R"
                | "W2R"
                | "WoW"
                | "WoWC"
                | "WoWE"
                | "WoWA"
                | "WTCG"
                | "D3"
                | "Fen"
                | "OSI"
        ) {
            finish(2);
        }
        code
    } else {
        finish(2);
    };
    let mode = lines.next().unwrap_or("");
    let mut key = NULL;
    let path = wide::<100>(match product {
        "Pro" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\Pro",
        "W3" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\W3",
        "W1R" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\W1R",
        "W2R" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\W2R",
        "WoW" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\WoW",
        "WoWC" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\WoW",
        "WoWE" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\WoW",
        "WoWA" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\WoW",
        "WTCG" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\WTCG",
        "D3" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\D3",
        "Fen" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\Fen",
        "OSI" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\OSI",
        "S1" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\S1",
        "Hero" => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\Hero",
        _ => "Software\\Blizzard Entertainment\\Battle.net\\Launch Options\\S2",
    });
    unsafe {
        if RegCreateKeyExW(
            0x80000001u32 as i32 as isize as Handle,
            path.as_ptr(),
            0,
            core::ptr::null_mut(),
            0,
            0x103,
            NULL,
            &mut key,
            core::ptr::null_mut(),
        ) != 0
        {
            finish(3);
        }
    }
    let name = wide::<12>("WEB_TOKEN");
    if mode == "clear" && lines.next() == Some("") && lines.next().is_none() {
        unsafe {
            let result = RegDeleteValueW(key, name.as_ptr());
            RegCloseKey(key);
            if result != 0 && result != 2 {
                finish(3);
            }
        }
        acknowledge(b"cleared\n");
        finish(87);
    }
    let entropy = Blob {
        size: 16,
        bytes: ENTROPY.as_ptr() as *mut u8,
    };
    let account_probe = if mode == "probe-account" {
        let account = lines.next().unwrap_or("");
        let region = lines.next().unwrap_or("");
        let locale = lines.next().unwrap_or("");
        if account.is_empty()
            || account.len() > 24
            || !account.bytes().all(|b| b.is_ascii_digit())
            || !matches!(region, "EU" | "US" | "KR" | "CN")
            || locale.len() != 4
            || !locale.bytes().all(|b| b.is_ascii_alphabetic())
            || lines.next() != Some("")
            || lines.next().is_some()
        {
            finish(2);
        }
        Some((account, region, locale))
    } else {
        None
    };
    let mut resume_build = "";
    let resume_root = if mode == "resume" {
        let root = lines.next().unwrap_or("");
        resume_build = lines.next().unwrap_or("");
        let valid_build = resume_build.is_empty()
            || (resume_build.starts_with("Base")
                && resume_build.len() > 4
                && resume_build.len() < 24
                && resume_build.as_bytes()[4..].iter().all(u8::is_ascii_digit));
        let tail = lines.next();
        if !root.starts_with("Z:\\")
            || root.contains(['"', '\r', '\0'])
            || root.encode_utf16().count() > 480
            || !valid_build
            || (tail != Some("") && tail.is_some())
            || lines.next().is_some()
        {
            finish(2);
        }
        Some(root)
    } else {
        None
    };
    if let Some(root) = resume_root {
        if matches!(product, "W1R" | "W2R") {
            unsafe {
                RegCloseKey(key);
            }
            start_game(root, "", product);
        }
    }
    if resume_root.is_some()
        || account_probe.is_some()
        || (mode == "probe" && lines.next() == Some("") && lines.next().is_none())
    {
        let mut encrypted = [0u8; 2048];
        let mut size = encrypted.len() as u32;
        let mut kind = 0;
        unsafe {
            let result = RegQueryValueExW(
                key,
                name.as_ptr(),
                core::ptr::null_mut(),
                &mut kind,
                encrypted.as_mut_ptr(),
                &mut size,
            );
            if result != 0 || kind != 3 {
                finish(4);
            }
            let data = Blob {
                size,
                bytes: encrypted.as_mut_ptr(),
            };
            let mut plain = Blob {
                size: 0,
                bytes: core::ptr::null_mut(),
            };
            if CryptUnprotectData(&data, NULL, &entropy, NULL, NULL, 1, &mut plain) == 0 {
                finish(4);
            }
            let bytes = core::slice::from_raw_parts_mut(plain.bytes, plain.size as usize);
            let valid = core::str::from_utf8(bytes)
                .map(|token| {
                    valid_token(token)
                        && account_probe
                            .is_none_or(|(account, _, _)| token.rsplit('-').next() == Some(account))
                })
                .unwrap_or(false);
            for b in bytes {
                core::ptr::write_volatile(b, 0);
            }
            LocalFree(plain.bytes as Handle);
            if !valid {
                finish(4);
            }
            if let Some((_, region, locale)) = account_probe {
                let region = wide::<3>(region);
                let locale = wide::<5>(locale);
                if RegSetValueExW(
                    key,
                    wide::<7>("REGION").as_ptr(),
                    0,
                    1,
                    region.as_ptr().cast(),
                    6,
                ) != 0
                    || RegSetValueExW(
                        key,
                        wide::<7>("LOCALE").as_ptr(),
                        0,
                        1,
                        locale.as_ptr().cast(),
                        10,
                    ) != 0
                {
                    finish(3);
                }
            }
            RegCloseKey(key);
        }
        if let Some(root) = resume_root {
            start_game(root, resume_build, product);
        }
        acknowledge(b"ticket-ready\n");
        finish(87);
    }
    if mode != "write" {
        finish(2);
    }
    let token = lines.next().unwrap_or("");
    let region = lines.next().unwrap_or("");
    let locale = lines.next().unwrap_or("");
    let root = lines.next().unwrap_or("");
    if !valid_token(token)
        || !matches!(region, "US" | "EU" | "KR" | "CN")
        || locale.len() != 4
        || !locale.bytes().all(|b| b.is_ascii_alphabetic())
        || root.len() > 500
        || root.contains(['"', '\r', '\0'])
        || lines.next() != Some("")
        || lines.next().is_some()
    {
        finish(2);
    }
    let plain = Blob {
        size: token.len() as u32,
        bytes: token.as_ptr() as *mut u8,
    };
    let mut protected = Blob {
        size: 0,
        bytes: core::ptr::null_mut(),
    };
    unsafe {
        if CryptProtectData(
            &plain,
            core::ptr::null(),
            &entropy,
            NULL,
            NULL,
            1,
            &mut protected,
        ) == 0
        {
            finish(5);
        }
        RegDeleteValueW(key, name.as_ptr());
        let region = wide::<3>(region);
        let locale = wide::<5>(locale);
        let mut result = RegSetValueExW(
            key,
            wide::<7>("REGION").as_ptr(),
            0,
            1,
            region.as_ptr() as *const u8,
            6,
        );
        result |= RegSetValueExW(
            key,
            wide::<7>("LOCALE").as_ptr(),
            0,
            1,
            locale.as_ptr() as *const u8,
            10,
        );
        if result == 0 {
            result = RegSetValueExW(key, name.as_ptr(), 0, 3, protected.bytes, protected.size);
        }
        for i in 0..protected.size as usize {
            core::ptr::write_volatile(protected.bytes.add(i), 0);
        }
        LocalFree(protected.bytes as Handle);
        if result != 0 {
            RegDeleteValueW(key, name.as_ptr());
            finish(3);
        }
        RegCloseKey(key);
        for b in &mut input {
            core::ptr::write_volatile(b, 0);
        }
    }
    acknowledge(b"stored\n");
    finish(87);
}
fn start_game(root: &str, build: &str, product: &str) -> ! {
    unsafe {
        let mut cmd = [0u16; 1400];
        let mut i = 0;
        let middle = match product {
            "Pro" => "\\_retail_\\Overwatch.exe",
            "W3" => "\\_retail_\\x86_64\\Warcraft III.exe",
            "W1R" => "\\x86\\Warcraft.exe",
            "W2R" => "\\x86\\Warcraft II.exe",
            "WoW" => "\\_retail_\\WoW.exe",
            "WoWC" => "\\_classic_\\WoWClassic.exe",
            "WoWE" => "\\_classic_era_\\WoWClassic.exe",
            "WoWA" => "\\_anniversary_\\WoWClassic.exe",
            "WTCG" => "\\Hearthstone.exe",
            "D3" => "\\x64\\Diablo III64.exe",
            "Fen" => "\\Diablo IV.exe",
            "OSI" => "\\D2R.exe",
            _ => {
                if product == "S1" {
                    "\\x86_64\\StarCraft.exe"
                } else if product == "Hero" {
                    "\\Support64\\HeroesSwitcher_x64.exe"
                } else if build.is_empty() {
                    "\\Support64\\SC2Switcher_x64.exe"
                } else {
                    "\\Versions\\"
                }
            }
        };
        let suffix = if build.is_empty() || product != "S2" {
            ""
        } else {
            "\\SC2_x64.exe"
        };
        let build = if product == "S2" { build } else { "" };
        let arguments = match product {
            "S1" => "\" -launch -uid s1",
            "D3" => "\" -launch OnlineService.SSO=true",
            "Fen" => "\" -nostreamline -sso -launch",
            "WoWA" => "\" -launch -initialgamemode=bccfresh",
            "Pro" | "W1R" | "W2R" | "OSI" => "\"",
            "W3" | "WoW" | "WoWC" | "WoWE" | "WTCG" => "\" -launch",
            "Hero" => "\" -sso=1 -launch -uid heroes",
            _ => "\" -sso=1 -launch -uid s2 -dataDir \"",
        };
        for c in "\""
            .encode_utf16()
            .chain(root.encode_utf16())
            .chain(middle.encode_utf16())
            .chain(build.encode_utf16())
            .chain(suffix.encode_utf16())
            .chain(arguments.encode_utf16())
            .chain(if product == "S2" { root } else { "" }.encode_utf16())
            .chain(if product == "S2" { "\\\\\"" } else { "" }.encode_utf16())
        {
            cmd[i] = c;
            i += 1;
        }
        let mut cwd = [0u16; 512];
        for (index, c) in root
            .encode_utf16()
            .chain(
                (if !build.is_empty() {
                    "\\Support64"
                } else {
                    match product {
                        "Pro" | "W3" | "WoW" => "\\_retail_",
                        "WoWC" => "\\_classic_",
                        "WoWE" => "\\_classic_era_",
                        "WoWA" => "\\_anniversary_",
                        _ => "",
                    }
                })
                .encode_utf16(),
            )
            .enumerate()
        {
            cwd[index] = c;
        }
        let mut startup: Startup = core::mem::zeroed();
        startup.size = core::mem::size_of::<Startup>() as u32;
        let mut process: Process = core::mem::zeroed();
        if CreateProcessW(
            core::ptr::null(),
            cmd.as_mut_ptr(),
            NULL,
            NULL,
            0,
            0,
            NULL,
            cwd.as_ptr(),
            &mut startup,
            &mut process,
        ) == 0
        {
            finish(6);
        }
        CloseHandle(process.thread);
        let game = if build.is_empty() && matches!(product, "S2" | "Hero") {
            let mut child = NULL;
            for _ in 0..450 {
                let snapshot = CreateToolhelp32Snapshot(2, 0);
                if snapshot as isize != -1 {
                    let mut entry: ProcessEntry = core::mem::zeroed();
                    entry.size = core::mem::size_of::<ProcessEntry>() as u32;
                    let mut more = Process32FirstW(snapshot, &mut entry);
                    while more != 0 {
                        let process_name = if product == "Hero" {
                            "HeroesOfTheStorm_x64.exe"
                        } else {
                            "SC2_x64.exe"
                        };
                        let name = wide::<260>(process_name);
                        let length = process_name.len() + 1;
                        let matches =
                            entry.executable[..length]
                                .iter()
                                .zip(&name[..length])
                                .all(|(a, b)| {
                                    (*a <= 127 && *b <= 127)
                                        && (*a as u8).to_ascii_lowercase()
                                            == (*b as u8).to_ascii_lowercase()
                                });
                        if entry.parent == process.pid && matches {
                            child = OpenProcess(0x100000 | 0x1000, 0, entry.pid);
                            break;
                        }
                        more = Process32NextW(snapshot, &mut entry);
                    }
                    CloseHandle(snapshot);
                }
                if !child.is_null() {
                    break;
                }
                Sleep(100);
            }
            CloseHandle(process.process);
            if child.is_null() {
                finish(6);
            }
            child
        } else {
            process.process
        };
        acknowledge(b"started\n");
        network_close();
        WaitForSingleObject(game, u32::MAX);
        let mut exit = 0;
        GetExitCodeProcess(game, &mut exit);
        CloseHandle(game);
        ExitProcess(exit);
    }
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(9)
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(p: *mut c_void, value: i32, n: usize) -> *mut c_void {
    for i in 0..n {
        unsafe {
            core::ptr::write_volatile(p.cast::<u8>().add(i), value as u8);
        }
    }
    p
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    for i in 0..n {
        unsafe {
            core::ptr::write_volatile(
                dst.cast::<u8>().add(i),
                core::ptr::read_volatile(src.cast::<u8>().add(i)),
            );
        }
    }
    dst
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    for i in 0..n {
        let x = unsafe { core::ptr::read_volatile(a.cast::<u8>().add(i)) };
        let y = unsafe { core::ptr::read_volatile(b.cast::<u8>().add(i)) };
        if x != y {
            return x as i32 - y as i32;
        }
    }
    0
}
// No unwinding or floating-point operations are used. Fail closed on SEH.
#[unsafe(no_mangle)]
pub extern "C" fn __CxxFrameHandler3() -> ! {
    finish(9)
}
#[unsafe(no_mangle)]
pub static _fltused: i32 = 0;
// Windows x64 stack probe ABI: allocation size in RAX, preserve RAX/RCX.
core::arch::global_asm!(
    r#"
.global __chkstk
__chkstk:
    push rcx
    push rax
    lea rcx, [rsp + 24]
    cmp rax, 4096
    jb 2f
1:
    sub rcx, 4096
    test byte ptr [rcx], 0
    sub rax, 4096
    cmp rax, 4096
    jae 1b
2:
    sub rcx, rax
    test byte ptr [rcx], 0
    pop rax
    pop rcx
    ret
"#
);
