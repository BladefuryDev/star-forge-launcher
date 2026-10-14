//! Agent DLL injected into game.exe by sc-inject (started from sc-launch).
//!
//! On load it reads the active profile (from `SC_PROFILE`), applies the
//! profile's byte patches to the game image, and hooks the Win32 file API so
//! any game file that also exists under `<profile>\files\` is opened from the
//! profile instead. The Steam install on disk is never modified.

#[cfg(windows)]
mod agent;
#[cfg(windows)]
mod hooks;
#[cfg(windows)]
mod patches;

#[cfg(windows)]
#[no_mangle]
pub extern "system" fn DllMain(
    module: windows_sys::Win32::Foundation::HINSTANCE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> windows_sys::Win32::Foundation::BOOL {
    use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
    if reason == DLL_PROCESS_ATTACH {
        unsafe { windows_sys::Win32::System::LibraryLoader::DisableThreadLibraryCalls(module) };
        // Never let a panic unwind into the loader; a failed init leaves the game unmodified.
        let _ = std::panic::catch_unwind(agent::init);
    }
    1
}
