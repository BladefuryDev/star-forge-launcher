//! Gives the game the profile's own `%LOCALAPPDATA%` (`<profile>\state\AppData\Local`).
//!
//! The game asks `SHGetFolderPathA(CSIDL_LOCAL_APPDATA)` once at startup and
//! keeps everything per-user under `Targem\StarConflict` there: the store
//! cache, player notes, logs, and the local server's MongoDB and Hub folders
//! (passed on to those processes). Answering with a profile folder keeps one
//! server's data out of another's sessions. The launcher copies the shared
//! settings files (`sc_profile::SHARED_STATE_FILES`) in and out.

use crate::agent::log;
use minhook::MinHook;
use std::ffi::c_void;
use std::sync::OnceLock;
use windows_sys::core::{PSTR, PWSTR};
use windows_sys::Win32::Foundation::{HANDLE, HWND};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type GetFolderAFn = unsafe extern "system" fn(HWND, i32, HANDLE, u32, PSTR) -> i32;
type GetFolderWFn = unsafe extern "system" fn(HWND, i32, HANDLE, u32, PWSTR) -> i32;

const CSIDL_LOCAL_APPDATA: i32 = 0x1c;
/// CSIDL values carry flags (`CSIDL_FLAG_CREATE`, ...) above the low byte.
const CSIDL_MASK: i32 = 0xff;
const MAX_PATH: usize = 260;

struct Folder {
    /// NUL-terminated.
    ansi: Vec<u8>,
    wide: Vec<u16>,
}

static FOLDER: OnceLock<Folder> = OnceLock::new();
static ORIG_A: OnceLock<GetFolderAFn> = OnceLock::new();
static ORIG_W: OnceLock<GetFolderWFn> = OnceLock::new();

unsafe extern "system" fn get_folder_a(hwnd: HWND, csidl: i32, token: HANDLE, flags: u32, path: PSTR) -> i32 {
    if csidl & CSIDL_MASK == CSIDL_LOCAL_APPDATA && !path.is_null() {
        if let Some(f) = FOLDER.get() {
            std::ptr::copy_nonoverlapping(f.ansi.as_ptr(), path, f.ansi.len());
            return 0;
        }
    }
    ORIG_A.get().unwrap_unchecked()(hwnd, csidl, token, flags, path)
}

unsafe extern "system" fn get_folder_w(hwnd: HWND, csidl: i32, token: HANDLE, flags: u32, path: PWSTR) -> i32 {
    if csidl & CSIDL_MASK == CSIDL_LOCAL_APPDATA && !path.is_null() {
        if let Some(f) = FOLDER.get() {
            std::ptr::copy_nonoverlapping(f.wide.as_ptr(), path, f.wide.len());
            return 0;
        }
    }
    ORIG_W.get().unwrap_unchecked()(hwnd, csidl, token, flags, path)
}

/// Points `CSIDL_LOCAL_APPDATA` at `dir`, creating it. On any problem the
/// game keeps the normal folder, and the log says why.
pub fn install(dir: &str) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log(&format!("game data folder {dir}: {e}; using the shared one"));
        return;
    }
    let Some(ansi) = crate::hooks::ansi(dir) else {
        log(&format!("game data folder {dir}: not representable in the system code page; using the shared one"));
        return;
    };
    let wide: Vec<u16> = dir.encode_utf16().chain(Some(0)).collect();
    if ansi.len() > MAX_PATH || wide.len() > MAX_PATH {
        log(&format!("game data folder {dir}: path too long for the game; using the shared one"));
        return;
    }
    let _ = FOLDER.set(Folder { ansi, wide });
    unsafe {
        let name = crate::hooks::wide("shell32.dll");
        let shell32 = LoadLibraryW(name.as_ptr());
        if shell32.is_null() {
            log("shell32.dll not found; using the shared game data folder");
            return;
        }
        let hooks: [(&[u8], *const ()); 2] =
            [(b"SHGetFolderPathA\0", get_folder_a as *const ()), (b"SHGetFolderPathW\0", get_folder_w as *const ())];
        for (i, (name, detour)) in hooks.into_iter().enumerate() {
            let Some(target) = GetProcAddress(shell32, name.as_ptr()) else {
                log("SHGetFolderPath not found; using the shared game data folder");
                return;
            };
            let target = target as *mut c_void;
            let trampoline = match MinHook::create_hook(target, detour as *mut c_void) {
                Ok(t) => t as *const (),
                Err(e) => {
                    log(&format!("hook SHGetFolderPath: {e:?}; using the shared game data folder"));
                    return;
                }
            };
            // The original must be in place before the hook can fire.
            if i == 0 {
                let _ = ORIG_A.set(std::mem::transmute::<*const (), GetFolderAFn>(trampoline));
            } else {
                let _ = ORIG_W.set(std::mem::transmute::<*const (), GetFolderWFn>(trampoline));
            }
            if let Err(e) = MinHook::enable_hook(target) {
                log(&format!("enabling SHGetFolderPath hook: {e:?}"));
                return;
            }
        }
    }
    log(&format!("game data folder: {dir}"));
}
