use crate::agent::log;
use sc_profile::Patch;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

pub fn apply_all(patches: &[Patch]) {
    for p in patches {
        let label = if p.description.is_empty() {
            format!("{}+{:#x}", p.module, p.offset)
        } else {
            format!("{} ({}+{:#x})", p.description, p.module, p.offset)
        };
        match unsafe { apply(p) } {
            Ok(msg) => log(&format!("patch {label}: {msg}")),
            Err(msg) => log(&format!("patch {label}: SKIPPED, {msg}")),
        }
    }
}

/// Size of the mapped image, from the PE optional header.
unsafe fn image_size(base: *const u8) -> usize {
    let nt = base.add(*(base.add(0x3c) as *const u32) as usize);
    *(nt.add(0x50) as *const u32) as usize
}

unsafe fn apply(p: &Patch) -> Result<&'static str, String> {
    let base = if p.module.eq_ignore_ascii_case("game.exe") {
        GetModuleHandleW(std::ptr::null())
    } else {
        let name: Vec<u16> = OsStr::new(&p.module).encode_wide().chain(Some(0)).collect();
        GetModuleHandleW(name.as_ptr())
    } as *mut u8;
    if base.is_null() {
        return Err(format!("{} is not loaded yet", p.module));
    }
    let len = p.original.len();
    if p.offset.checked_add(len).map_or(true, |end| end > image_size(base)) {
        return Err("offset is outside the module".into());
    }
    let target = base.add(p.offset);
    let current = std::slice::from_raw_parts(target, len);
    if current == p.patched.as_slice() {
        return Ok("already applied");
    }
    if current != p.original.as_slice() {
        return Err(format!("expected bytes {:02X?}, found {:02X?}", p.original, current));
    }
    let mut old = 0u32;
    if VirtualProtect(target.cast(), len, PAGE_EXECUTE_READWRITE, &mut old) == 0 {
        return Err("VirtualProtect failed".into());
    }
    std::ptr::copy_nonoverlapping(p.patched.as_ptr(), target, len);
    VirtualProtect(target.cast(), len, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), target.cast(), len);
    Ok("applied")
}
