#![no_std]
#![no_main]
use core::{ffi::c_void, panic::PanicInfo};
type Handle = *mut c_void;
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
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    fn GetModuleFileNameW(module: *mut c_void, buffer: *mut u16, size: u32) -> u32;
    fn Sleep(milliseconds: u32);
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn CreateProcessW(
        app: *const u16,
        cmd: *mut u16,
        pa: *mut c_void,
        ta: *mut c_void,
        inherit: i32,
        flags: u32,
        env: *mut c_void,
        cwd: *const u16,
        startup: *mut Startup,
        process: *mut Process,
    ) -> i32;
    fn GetCurrentDirectoryW(size: u32, buffer: *mut u16) -> u32;
    fn GetCommandLineW() -> *const u16;
    fn ExitProcess(code: u32) -> !;
}
#[unsafe(no_mangle)]
pub extern "system" fn mainCRTStartup() -> ! {
    unsafe {
        let mut module = [0u16; 512];
        let n = GetModuleFileNameW(core::ptr::null_mut(), module.as_mut_ptr(), 512) as usize;
        let ending: [u16; 29] = {
            let mut out = [0; 29];
            let text = b"Support64\\SC2Switcher_x64.exe";
            let mut i = 0;
            while i < text.len() {
                out[i] = text[i] as u16;
                i += 1;
            }
            out
        };
        if n >= ending.len() && module[n - ending.len()..n] == ending {
            let root_len = n - ending.len() - 1;
            let mut executable = [0u16; 600];
            executable[..root_len].copy_from_slice(&module[..root_len]);
            let suffix = "\\Versions\\Base12345\\SC2_x64.exe";
            for (i, c) in suffix.encode_utf16().enumerate() {
                executable[root_len + i] = c;
            }
            let mut cwd = [0u16; 600];
            cwd[..root_len].copy_from_slice(&module[..root_len]);
            for (i, c) in "\\Support64".encode_utf16().enumerate() {
                cwd[root_len + i] = c;
            }
            let mut command = [0u16; 700];
            let mut i = 0;
            command[i] = 34;
            i += 1;
            for c in executable.iter().copied().take_while(|c| *c != 0) {
                command[i] = c;
                i += 1;
            }
            for c in "\" --fixture-child".encode_utf16() {
                command[i] = c;
                i += 1;
            }
            let mut startup: Startup = core::mem::zeroed();
            startup.size = core::mem::size_of::<Startup>() as u32;
            let mut process: Process = core::mem::zeroed();
            if CreateProcessW(
                executable.as_ptr(),
                command.as_mut_ptr(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                0,
                0,
                core::ptr::null_mut(),
                cwd.as_ptr(),
                &mut startup,
                &mut process,
            ) == 0
            {
                ExitProcess(53);
            }
            CloseHandle(process.thread);
            CloseHandle(process.process);
            ExitProcess(0);
        }
        let mut cwd = [0u16; 512];
        let n = GetCurrentDirectoryW(512, cwd.as_mut_ptr()) as usize;
        let suffix = b"\\Support64";
        if n >= 512 || n < suffix.len() {
            ExitProcess(51);
        }
        for i in 0..suffix.len() {
            if cwd[n - suffix.len() + i] != suffix[i] as u16 {
                ExitProcess(51);
            }
        }
        let cmd = GetCommandLineW();
        let child = b"--fixture-child";
        for i in 0..700 {
            if *cmd.add(i) == 0 {
                break;
            }
            if child
                .iter()
                .enumerate()
                .all(|(j, b)| *cmd.add(i + j) == *b as u16)
            {
                Sleep(900);
                ExitProcess(47);
            }
        }
        let needle = b"-dataDir \"";
        let mut found = false;
        for i in 0..1400 {
            if *cmd.add(i) == 0 {
                break;
            }
            let mut same = true;
            for j in 0..needle.len() {
                if *cmd.add(i + j) != needle[j] as u16 {
                    same = false;
                    break;
                }
            }
            if same {
                let root_len = n - suffix.len();
                for j in 0..root_len {
                    if *cmd.add(i + needle.len() + j) != cwd[j] {
                        ExitProcess(52);
                    }
                }
                let ending = [b'\\', b'\\', b'"'];
                if !ending
                    .iter()
                    .enumerate()
                    .all(|(j, byte)| *cmd.add(i + needle.len() + root_len + j) == *byte as u16)
                {
                    ExitProcess(52);
                }
                found = true;
                break;
            }
        }
        if !found {
            ExitProcess(52);
        }
        ExitProcess(47);
    }
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    unsafe { ExitProcess(59) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(p: *mut c_void, v: i32, n: usize) -> *mut c_void {
    for i in 0..n {
        unsafe {
            core::ptr::write_volatile(p.cast::<u8>().add(i), v as u8);
        }
    }
    p
}
#[unsafe(no_mangle)]
pub static _fltused: i32 = 0;
#[unsafe(no_mangle)]
pub extern "C" fn __CxxFrameHandler3() -> ! {
    unsafe { ExitProcess(59) }
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
