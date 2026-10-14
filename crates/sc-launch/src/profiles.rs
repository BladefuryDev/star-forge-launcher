//! Where profiles live and how they're named.

use sc_profile::{Profile, PROFILE_FILE};
use std::path::{Path, PathBuf};

/// Folder name under the platform data folder; matches the Tauri identifier so
/// the webview's own storage lands in the same place.
const DATA_FOLDER: &str = "star-forge";
/// Name used before the STAR FORGE rename; its data is moved over on first use.
const OLD_DATA_FOLDER: &str = "sc-launcher";

fn platform_data_root() -> Option<PathBuf> {
    #[cfg(windows)]
    return std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    return std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    return std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
}

/// Per-user data folder for the launcher (`STAR_FORGE_DATA_DIR` overrides it).
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = crate::env::path(crate::env::DATA_DIR) {
        return Some(dir);
    }
    let root = platform_data_root()?;
    let dir = root.join(DATA_FOLDER);
    let old = root.join(OLD_DATA_FOLDER);
    if !dir.exists() && old.is_dir() {
        // Best effort: if the move fails the launcher just starts with fresh data.
        let _ = std::fs::rename(&old, &dir);
    }
    Some(dir)
}

pub fn default_profiles_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("profiles"))
}

/// A profile name is a single folder name: no separators, no dot names.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
}

pub struct Entry {
    pub name: String,
    pub dir: PathBuf,
    pub profile: Result<Profile, String>,
}

pub fn load(dir: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(dir.join(PROFILE_FILE))
        .map_err(|e| format!("{}: {e}", dir.join(PROFILE_FILE).display()))?;
    let profile = Profile::from_toml(&text).map_err(|e| e.to_string())?;
    profile.patches().map_err(|e| e.to_string())?;
    profile.address_cfg().map_err(|e| e.to_string())?;
    Ok(profile)
}

pub fn list(profiles_dir: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = std::fs::read_dir(profiles_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join(PROFILE_FILE).is_file())
        .map(|e| {
            let dir = e.path();
            Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                profile: load(&dir),
                dir,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn names() {
        assert!(valid_name("starfury"));
        assert!(valid_name("local dev"));
        assert!(!valid_name(".."));
        assert!(!valid_name("a/b"));
        assert!(!valid_name(r"a\b"));
        assert!(!valid_name("C:x"));
    }
}
