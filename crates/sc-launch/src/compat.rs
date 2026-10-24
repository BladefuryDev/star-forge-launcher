//! Running the Windows injector through Proton on Linux (macOS: future work).
//!
//! By default this reuses the game's own Steam prefix (`compatdata/212070`) and
//! the Proton build that last ran it, so settings, the local Mongo data and the
//! prefix version stay the same as a normal Steam launch.
//!
//! Proton always runs inside the Steam Linux Runtime its build asks for
//! (`require_tool_appid` in its `toolmanifest.vdf`), through the runtime's
//! `run` script, as Steam does. A build that names no runtime, or whose
//! runtime isn't installed, is refused rather than run against the host's
//! libraries; the error says what to do in Steam.

use crate::launch::{SetupError, SteamFix};
use crate::steam::{find_app, vdf_values};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Proton {
    /// Folder holding the `proton` script.
    pub dir: PathBuf,
    /// Steam Linux Runtime `run` script the build asks for.
    pub runtime: PathBuf,
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
pub fn runtime_for(proton_dir: &Path, libraries: &[PathBuf]) -> Result<PathBuf, SetupError> {
    let name = proton_dir.file_name().map_or_else(|| proton_dir.display().to_string(), |n| n.to_string_lossy().into_owned());
    let appid = std::fs::read_to_string(proton_dir.join("toolmanifest.vdf"))
        .ok()
        .and_then(|m| vdf_values(&m, "require_tool_appid").into_iter().next())
        .and_then(|id| id.parse::<u32>().ok())
        .ok_or_else(|| {
            SetupError::new(
                format!(
                    "{name} doesn't run inside the Steam Linux Runtime, so the launcher won't use it. \
                     Choose a current Proton for Star Conflict in Steam (Properties → Compatibility), \
                     start the game once from Steam, then retry."
                ),
                None,
            )
        })?;
    let run = find_app(libraries, &appid.to_string()).map(|(_, dir)| dir.join("run")).filter(|r| r.is_file());
    run.ok_or_else(|| {
        SetupError::new(
            format!("{name} needs the Steam Linux Runtime (Steam app {appid}), which isn't installed. Install it from Steam, then retry."),
            Some(SteamFix::InstallRuntime(appid)),
        )
    })
}

pub fn detect(
    steam_root: &Path,
    game_library: &Path,
    libraries: &[PathBuf],
    proton_override: Option<&Path>,
) -> Result<Proton, SetupError> {
    let compat_data = game_library.join("steamapps/compatdata").join(crate::steam::GAME_APPID);
    let start_once = |what: &str| {
        SetupError::new(
            format!("{what} Start Star Conflict once from Steam so it sets up Proton and the game's prefix, close it, then retry."),
            Some(SteamFix::StartOnce),
        )
    };
    // Steam builds the prefix (and runs the game's first-time installers);
    // the launcher only ever reuses it.
    if !compat_data.join("pfx").is_dir() {
        return Err(start_once("Star Conflict hasn't been started from Steam on this computer yet."));
    }
    let dir = match proton_override {
        Some(p) => p.to_path_buf(),
        None => std::fs::read_to_string(compat_data.join("config_info"))
            .ok()
            .and_then(|t| proton_from_config_info(&t))
            .ok_or_else(|| start_once("Steam hasn't recorded which Proton runs Star Conflict."))?,
    };
    if !dir.join("proton").is_file() {
        return Err(match proton_override {
            Some(_) => SetupError::new(format!("{} has no proton script; choose another Proton in Settings.", dir.display()), None),
            None => start_once(&format!("The Proton build Steam last used for Star Conflict ({}) is gone.", dir.display())),
        });
    }
    let runtime = runtime_for(&dir, libraries)?;
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

    #[test]
    fn proton_must_name_an_installed_runtime() {
        let base = std::env::temp_dir().join(format!("sf-compat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let library = base.join("library");
        let proton = base.join("proton-x");
        std::fs::create_dir_all(library.join("steamapps/common/SteamLinuxRuntime_4")).unwrap();
        std::fs::create_dir_all(&proton).unwrap();
        let libraries = vec![library.clone()];

        let err = runtime_for(&proton, &libraries).unwrap_err();
        assert_eq!(err.fix, None, "no toolmanifest: refused, nothing to install");

        std::fs::write(proton.join("toolmanifest.vdf"), "\"manifest\"\n{\n  \"require_tool_appid\" \"4183110\"\n}\n").unwrap();
        let err = runtime_for(&proton, &libraries).unwrap_err();
        assert_eq!(err.fix, Some(SteamFix::InstallRuntime(4183110)));
        assert_eq!(SteamFix::InstallRuntime(4183110).url(), "steam://install/4183110");

        std::fs::write(
            library.join("steamapps/appmanifest_4183110.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"4183110\"\n\t\"installdir\"\t\t\"SteamLinuxRuntime_4\"\n}",
        )
        .unwrap();
        let run = library.join("steamapps/common/SteamLinuxRuntime_4/run");
        std::fs::write(&run, "").unwrap();
        assert_eq!(runtime_for(&proton, &libraries).unwrap(), run);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
