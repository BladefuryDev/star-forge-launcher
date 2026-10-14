//! File API detours. A path inside the game root is replaced with the profile's
//! copy when `<profile>\files\<same relative path>` exists, and folder
//! listings include files that only exist in the profile (the engine finds
//! loose files by listing `data\`, so they must show up there to be loaded).

use crate::agent::{log, state};
use minhook::MinHook;
use sc_profile::{profile_file, relative_to};
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use windows_sys::core::{PCSTR, PCWSTR};
use windows_sys::Win32::Foundation::{
    SetLastError, BOOL, ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, FALSE, HANDLE, INVALID_HANDLE_VALUE, TRUE,
};
use windows_sys::Win32::Globalization::{MultiByteToWideChar, WideCharToMultiByte, CP_ACP};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    GetFullPathNameW, FILE_ATTRIBUTE_DIRECTORY, INVALID_FILE_ATTRIBUTES, WIN32_FIND_DATAA,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

type CreateFileWFn =
    unsafe extern "system" fn(PCWSTR, u32, u32, *const SECURITY_ATTRIBUTES, u32, u32, HANDLE) -> HANDLE;
type CreateFileAFn =
    unsafe extern "system" fn(PCSTR, u32, u32, *const SECURITY_ATTRIBUTES, u32, u32, HANDLE) -> HANDLE;
type GetAttrsWFn = unsafe extern "system" fn(PCWSTR) -> u32;
type GetAttrsAFn = unsafe extern "system" fn(PCSTR) -> u32;
type GetAttrsExWFn = unsafe extern "system" fn(PCWSTR, i32, *mut c_void) -> BOOL;
type GetAttrsExAFn = unsafe extern "system" fn(PCSTR, i32, *mut c_void) -> BOOL;
type FindFirstAFn = unsafe extern "system" fn(PCSTR, *mut WIN32_FIND_DATAA) -> HANDLE;
type FindFirstExAFn = unsafe extern "system" fn(PCSTR, i32, *mut c_void, i32, *const c_void, u32) -> HANDLE;
type FindNextAFn = unsafe extern "system" fn(HANDLE, *mut WIN32_FIND_DATAA) -> BOOL;
type FindCloseFn = unsafe extern "system" fn(HANDLE) -> BOOL;

/// Original (unhooked) entry points, captured before any detour is enabled.
struct Originals {
    create_w: CreateFileWFn,
    create_a: CreateFileAFn,
    attrs_w: GetAttrsWFn,
    attrs_a: GetAttrsAFn,
    attrs_ex_w: GetAttrsExWFn,
    attrs_ex_a: GetAttrsExAFn,
    find_first_a: FindFirstAFn,
    find_first_ex_a: FindFirstExAFn,
    find_next_a: FindNextAFn,
    find_close: FindCloseFn,
}

/// Merged listings handed out under our own handles. The key is the address of
/// the boxed token, which can't equal any live handle the system gives out.
struct Listing {
    _token: Box<u8>,
    rest: VecDeque<WIN32_FIND_DATAA>,
}

static LISTINGS: OnceLock<Mutex<HashMap<usize, Listing>>> = OnceLock::new();

fn listings() -> &'static Mutex<HashMap<usize, Listing>> {
    LISTINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

static ORIG: OnceLock<Originals> = OnceLock::new();

thread_local! {
    static IN_HOOK: Cell<bool> = const { Cell::new(false) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

unsafe fn wstr(p: PCWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)))
}

unsafe fn ansi_to_wide(p: PCSTR) -> Option<Vec<u16>> {
    if p.is_null() {
        return None;
    }
    let n = MultiByteToWideChar(CP_ACP, 0, p, -1, std::ptr::null_mut(), 0);
    if n <= 0 {
        return None;
    }
    let mut buf = vec![0u16; n as usize];
    MultiByteToWideChar(CP_ACP, 0, p, -1, buf.as_mut_ptr(), n);
    Some(buf)
}

fn ansi(s: &str) -> Option<Vec<u8>> {
    let w = wide(s);
    unsafe {
        let n = WideCharToMultiByte(CP_ACP, 0, w.as_ptr(), -1, std::ptr::null_mut(), 0, std::ptr::null(), std::ptr::null_mut());
        if n <= 0 {
            return None;
        }
        let mut buf = vec![0u8; n as usize];
        WideCharToMultiByte(CP_ACP, 0, w.as_ptr(), -1, buf.as_mut_ptr(), n, std::ptr::null(), std::ptr::null_mut());
        Some(buf)
    }
}

unsafe fn full_path(p: PCWSTR) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let n = GetFullPathNameW(p, buf.len() as u32, buf.as_mut_ptr(), std::ptr::null_mut());
    if n == 0 || n as usize >= buf.len() {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..n as usize]))
}

/// Returns the profile replacement for `path` (NUL-terminated wide), if any.
unsafe fn redirect(path: PCWSTR) -> Option<Vec<u16>> {
    if IN_HOOK.with(|g| g.replace(true)) {
        return None;
    }
    let result = (|| {
        let s = state()?;
        let orig = ORIG.get()?;
        wstr(path)?;
        let full = full_path(path)?;
        let traced = s.trace.load(std::sync::atomic::Ordering::Relaxed);
        let rel = relative_to(&s.game_root, &full)?;
        let key = rel.to_ascii_lowercase();
        let mut cache = s.redirects.lock().ok()?;
        if let Some(hit) = cache.get(&key) {
            return hit.clone();
        }
        if traced {
            log(&format!("trace {rel}"));
        }
        let candidate = profile_file(&s.profile_dir, &rel);
        let candidate_w = wide(&candidate);
        let attrs = (orig.attrs_w)(candidate_w.as_ptr());
        let hit = (attrs != INVALID_FILE_ATTRIBUTES && attrs & FILE_ATTRIBUTE_DIRECTORY == 0)
            .then_some(candidate_w);
        if hit.is_some() {
            log(&format!("redirect {rel} -> {candidate}"));
        }
        cache.insert(key, hit.clone());
        hit
    })();
    IN_HOOK.with(|g| g.set(false));
    result
}

unsafe extern "system" fn create_file_w(
    name: PCWSTR, access: u32, share: u32, sa: *const SECURITY_ATTRIBUTES, disp: u32, flags: u32, tmpl: HANDLE,
) -> HANDLE {
    let o = ORIG.get().unwrap_unchecked();
    match redirect(name) {
        Some(p) => (o.create_w)(p.as_ptr(), access, share, sa, disp, flags, tmpl),
        None => (o.create_w)(name, access, share, sa, disp, flags, tmpl),
    }
}

unsafe extern "system" fn create_file_a(
    name: PCSTR, access: u32, share: u32, sa: *const SECURITY_ATTRIBUTES, disp: u32, flags: u32, tmpl: HANDLE,
) -> HANDLE {
    let o = ORIG.get().unwrap_unchecked();
    if let Some(p) = ansi_to_wide(name).and_then(|w| redirect(w.as_ptr())) {
        return (o.create_w)(p.as_ptr(), access, share, sa, disp, flags, tmpl);
    }
    (o.create_a)(name, access, share, sa, disp, flags, tmpl)
}

unsafe extern "system" fn get_attrs_w(name: PCWSTR) -> u32 {
    let o = ORIG.get().unwrap_unchecked();
    match redirect(name) {
        Some(p) => (o.attrs_w)(p.as_ptr()),
        None => (o.attrs_w)(name),
    }
}

unsafe extern "system" fn get_attrs_a(name: PCSTR) -> u32 {
    let o = ORIG.get().unwrap_unchecked();
    if let Some(p) = ansi_to_wide(name).and_then(|w| redirect(w.as_ptr())) {
        return (o.attrs_w)(p.as_ptr());
    }
    (o.attrs_a)(name)
}

unsafe extern "system" fn get_attrs_ex_w(name: PCWSTR, level: i32, info: *mut c_void) -> BOOL {
    let o = ORIG.get().unwrap_unchecked();
    match redirect(name) {
        Some(p) => (o.attrs_ex_w)(p.as_ptr(), level, info),
        None => (o.attrs_ex_w)(name, level, info),
    }
}

unsafe extern "system" fn get_attrs_ex_a(name: PCSTR, level: i32, info: *mut c_void) -> BOOL {
    let o = ORIG.get().unwrap_unchecked();
    if let Some(p) = ansi_to_wide(name).and_then(|w| redirect(w.as_ptr())) {
        return (o.attrs_ex_w)(p.as_ptr(), level, info);
    }
    (o.attrs_ex_a)(name, level, info)
}

/// Lower-cased entry name, for matching profile entries against real ones.
fn entry_name(d: &WIN32_FIND_DATAA) -> Vec<u8> {
    d.cFileName
        .iter()
        .map(|&c| c as u8)
        .take_while(|&c| c != 0)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Every entry of a search, via the unhooked functions.
unsafe fn collect(first: HANDLE, data: WIN32_FIND_DATAA, out: &mut Vec<WIN32_FIND_DATAA>) {
    let o = ORIG.get().unwrap_unchecked();
    if first == INVALID_HANDLE_VALUE {
        return;
    }
    let mut d = data;
    loop {
        out.push(d);
        if (o.find_next_a)(first, &mut d) == FALSE {
            break;
        }
    }
    (o.find_close)(first);
}

/// For a search pattern under the game root whose folder also exists in the
/// profile: the real entries plus the profile's (profile wins on a name clash).
/// `real` runs the caller's own search. None means "not ours, pass through".
unsafe fn merged(pattern: PCSTR, real: impl FnOnce(&mut WIN32_FIND_DATAA) -> HANDLE) -> Option<Vec<WIN32_FIND_DATAA>> {
    let s = state()?;
    let o = ORIG.get()?;
    let full = full_path(ansi_to_wide(pattern)?.as_ptr())?;
    let (dir, name) = full.rsplit_once('\\')?;
    let rel_dir = relative_to(&s.game_root, dir)?;
    if s.trace.load(std::sync::atomic::Ordering::Relaxed) {
        log(&format!("trace list {rel_dir}\\{name}"));
    }
    let prof_dir = profile_file(&s.profile_dir, &rel_dir);
    let attrs = (o.attrs_w)(wide(&prof_dir).as_ptr());
    if attrs == INVALID_FILE_ATTRIBUTES || attrs & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return None;
    }

    let mut extra = Vec::new();
    let prof_pattern = ansi(&format!("{prof_dir}\\{name}"))?;
    let mut d: WIN32_FIND_DATAA = std::mem::zeroed();
    let h = (o.find_first_a)(prof_pattern.as_ptr(), &mut d);
    collect(h, d, &mut extra);
    extra.retain(|e| !matches!(entry_name(e).as_slice(), b"." | b".."));

    let mut out = Vec::new();
    let mut d: WIN32_FIND_DATAA = std::mem::zeroed();
    let h = real(&mut d);
    collect(h, d, &mut out);
    out.retain(|e| !extra.iter().any(|x| entry_name(x) == entry_name(e)));
    for e in &extra {
        log(&format!("list {rel_dir}: adding {}", String::from_utf8_lossy(&entry_name(e))));
    }
    out.extend(extra);
    Some(out)
}

/// Hands out a merged listing: the first entry now, the rest via FindNextFileA.
unsafe fn start_listing(mut entries: Vec<WIN32_FIND_DATAA>, data: *mut WIN32_FIND_DATAA) -> HANDLE {
    if entries.is_empty() {
        SetLastError(ERROR_FILE_NOT_FOUND);
        return INVALID_HANDLE_VALUE;
    }
    *data = entries.remove(0);
    let token = Box::new(0u8);
    let handle = &*token as *const u8 as usize;
    if let Ok(mut map) = listings().lock() {
        map.insert(handle, Listing { _token: token, rest: entries.into() });
    }
    handle as HANDLE
}

/// Runs `f` with the reentrancy guard set (so our own calls aren't hooked).
fn guarded<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
    if IN_HOOK.with(|g| g.replace(true)) {
        return None;
    }
    let r = f();
    IN_HOOK.with(|g| g.set(false));
    r
}

unsafe extern "system" fn find_first_a(name: PCSTR, data: *mut WIN32_FIND_DATAA) -> HANDLE {
    let o = ORIG.get().unwrap_unchecked();
    match guarded(|| merged(name, |d| (o.find_first_a)(name, d))) {
        Some(entries) => start_listing(entries, data),
        None => (o.find_first_a)(name, data),
    }
}

unsafe extern "system" fn find_first_ex_a(
    name: PCSTR, level: i32, data: *mut c_void, op: i32, filter: *const c_void, flags: u32,
) -> HANDLE {
    let o = ORIG.get().unwrap_unchecked();
    let real = |d: &mut WIN32_FIND_DATAA| (o.find_first_ex_a)(name, level, d as *mut _ as *mut c_void, op, filter, flags);
    match guarded(|| merged(name, real)) {
        Some(entries) => start_listing(entries, data.cast()),
        None => (o.find_first_ex_a)(name, level, data, op, filter, flags),
    }
}

unsafe extern "system" fn find_next_a(h: HANDLE, data: *mut WIN32_FIND_DATAA) -> BOOL {
    let o = ORIG.get().unwrap_unchecked();
    if let Ok(mut map) = listings().lock() {
        if let Some(l) = map.get_mut(&(h as usize)) {
            return match l.rest.pop_front() {
                Some(d) => {
                    *data = d;
                    TRUE
                }
                None => {
                    SetLastError(ERROR_NO_MORE_FILES);
                    FALSE
                }
            };
        }
    }
    (o.find_next_a)(h, data)
}

unsafe extern "system" fn find_close(h: HANDLE) -> BOOL {
    let o = ORIG.get().unwrap_unchecked();
    if let Ok(mut map) = listings().lock() {
        if map.remove(&(h as usize)).is_some() {
            return TRUE;
        }
    }
    (o.find_close)(h)
}

/// Finds an export in kernelbase (where the real code lives on Windows 8+ and
/// Wine), falling back to kernel32.
unsafe fn export(name: &str) -> Option<*const ()> {
    let cname = format!("{name}\0");
    for dll in ["kernelbase.dll", "kernel32.dll"] {
        let m = GetModuleHandleW(wide(dll).as_ptr());
        if m.is_null() {
            continue;
        }
        if let Some(f) = GetProcAddress(m, cname.as_ptr()) {
            return Some(f as *const ());
        }
    }
    None
}

pub fn install() {
    unsafe {
        let names = [
            ("CreateFileW", create_file_w as *const ()),
            ("CreateFileA", create_file_a as *const ()),
            ("GetFileAttributesW", get_attrs_w as *const ()),
            ("GetFileAttributesA", get_attrs_a as *const ()),
            ("GetFileAttributesExW", get_attrs_ex_w as *const ()),
            ("GetFileAttributesExA", get_attrs_ex_a as *const ()),
            ("FindFirstFileA", find_first_a as *const ()),
            ("FindFirstFileExA", find_first_ex_a as *const ()),
            ("FindNextFileA", find_next_a as *const ()),
            ("FindClose", find_close as *const ()),
        ];
        let mut trampolines = Vec::new();
        for (name, hook) in names {
            let Some(target) = export(name) else {
                log(&format!("hook {name}: export not found, file redirects disabled"));
                return;
            };
            match MinHook::create_hook(target as *mut c_void, hook as *mut c_void) {
                Ok(t) => trampolines.push(t as *const ()),
                Err(e) => {
                    log(&format!("hook {name}: {e:?}, file redirects disabled"));
                    return;
                }
            }
        }
        let t = |i: usize| trampolines[i];
        // Originals must be in place before any hook can fire.
        let _ = ORIG.set(Originals {
            create_w: std::mem::transmute::<*const (), CreateFileWFn>(t(0)),
            create_a: std::mem::transmute::<*const (), CreateFileAFn>(t(1)),
            attrs_w: std::mem::transmute::<*const (), GetAttrsWFn>(t(2)),
            attrs_a: std::mem::transmute::<*const (), GetAttrsAFn>(t(3)),
            attrs_ex_w: std::mem::transmute::<*const (), GetAttrsExWFn>(t(4)),
            attrs_ex_a: std::mem::transmute::<*const (), GetAttrsExAFn>(t(5)),
            find_first_a: std::mem::transmute::<*const (), FindFirstAFn>(t(6)),
            find_first_ex_a: std::mem::transmute::<*const (), FindFirstExAFn>(t(7)),
            find_next_a: std::mem::transmute::<*const (), FindNextAFn>(t(8)),
            find_close: std::mem::transmute::<*const (), FindCloseFn>(t(9)),
        });
        match MinHook::enable_all_hooks() {
            Ok(()) => log("file hooks installed"),
            Err(e) => log(&format!("enabling hooks failed: {e:?}")),
        }
    }
}
