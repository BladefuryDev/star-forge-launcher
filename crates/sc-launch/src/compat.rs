//! Running the Windows injector through Proton on Linux (macOS: future work).
//!
//! By default this reuses the game's own Steam prefix (`compatdata/212070`) and
//! the Proton build that last ran it, so settings, the local Mongo data and the
//! prefix version stay the same as a normal Steam launch.

use crate::steam::{find_app, vdf_values};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Proton {
    /// Folder holding the `proton` script.
    pub dir: PathBuf,
    /// Steam Linux Runtime `run` script the build asks for, if installed.
    pub runtime: Option<PathBuf>,
    /// STEAM_COMPAT_DATA_PATH (the folder that holds `pfx/`).
    pub compat_data: PathBuf,
    /// STEAM_COMPAT_CLIENT_INSTALL_PATH.
    pub steam_root: PathBuf,
}

/// Proton folder recorded in a prefix's `config_info` (line 2 is a path inside
/// the build, e.g. `<proton>/files/share/fonts/`).
pub fn proton_from_config_info(text: &str) -> Option<PathBuf> {
    text.lines()
        .skip(1)
        .find_map(|l| l.split_once("/files/").map(|(dir, _)| PathBuf::from(dir)))
}

/// Every Proton build under the Steam libraries and the usual tool folders.
pub fn proton_candidates(roots: &[PathBuf], libraries: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = libraries.iter().map(|l| l.join("steamapps/common")).collect();
    dirs.extend(roots.iter().map(|r| r.join("compatibilitytools.d")));
    dirs.push(PathBuf::from("/usr/share/steam/compatibilitytools.d"));
    let mut out = Vec::new();
    for d in dirs {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.join("proton").is_file() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// The runtime `run` script a Proton build requires via `require_tool_appid`.
pub fn runtime_for(proton_dir: &Path, libraries: &[PathBuf]) -> Option<PathBuf> {
    let manifest = std::fs::read_to_string(proton_dir.join("toolmanifest.vdf")).ok()?;
    let appid = vdf_values(&manifest, "require_tool_appid").into_iter().next()?;
    let (_, dir) = find_app(libraries, &appid)?;
    let run = dir.join("run");
    run.is_file().then_some(run)
}

pub fn detect(
    steam_root: &Path,
    game_library: &Path,
    libraries: &[PathBuf],
    proton_override: Option<&Path>,
) -> Result<Proton, String> {
    let compat_data = game_library.join("steamapps/compatdata").join(crate::steam::GAME_APPID);
    let dir = match proton_override {
        Some(p) => p.to_path_buf(),
        None => std::fs::read_to_string(compat_data.join("config_info"))
            .ok()
            .and_then(|t| proton_from_config_info(&t))
            .ok_or_else(|| {
                format!(
                    "could not tell which Proton runs {} (launch the game from Steam once, or pass --proton)",
                    compat_data.display()
                )
            })?,
    };
    if !dir.join("proton").is_file() {
        return Err(format!("{} has no proton script", dir.display()));
    }
    let runtime = runtime_for(&dir, libraries);
    Ok(Proton {
        dir,
        runtime,
        compat_data,
        steam_root: steam_root.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_info_gives_the_proton_folder() {
        let text = "CachyOS-11.0-100\n/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/share/fonts/\n/x/files/lib/\n";
        assert_eq!(
            proton_from_config_info(text),
            Some(PathBuf::from("/usr/share/steam/compatibilitytools.d/proton-cachyos-slr"))
        );
        assert_eq!(proton_from_config_info("only-a-version\n"), None);
    }
}
