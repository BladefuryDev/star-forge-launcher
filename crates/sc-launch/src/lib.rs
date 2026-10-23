//! Native (Linux/Windows) side of the launcher: finds the game and Proton,
//! manages profiles, and starts sc-inject.exe. Kept as a library so a GUI can
//! wrap it later.

#[cfg(unix)]
pub mod compat;
pub mod env;
pub mod launch;
pub mod packs;
pub mod patching;
pub mod profiles;
pub mod remote;
pub mod signing;
pub mod state;
pub mod steam;
pub mod trust;

/// One launcher data folder for every test in the process (tests run in
/// parallel threads and `STAR_FORGE_DATA_DIR` is process-wide).
#[cfg(test)]
pub(crate) fn test_data_dir() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("sf-test-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var(env::DATA_DIR, &dir);
        dir
    })
}
