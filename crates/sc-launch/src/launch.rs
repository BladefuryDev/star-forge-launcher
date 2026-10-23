//! Works out the game, tools and (on Linux) Proton setup, and builds the command
//! that starts sc-inject.exe for a profile.

use crate::steam;
#[cfg(unix)]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const INJECT_EXE: &str = "sc-inject.exe";
pub const AGENT_DLL: &str = "sc_agent.dll";

#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Game install folder (holding win64\ and data\). Found through Steam if unset.
    pub game_dir: Option<PathBuf>,
    /// Folder holding sc-inject.exe and sc_agent.dll. Defaults to this program's folder.
    pub tools_dir: Option<PathBuf>,
    /// Proton build folder to use instead of the one that last ran the game's prefix.
    pub proton: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Setup {
    pub game_dir: PathBuf,
    pub tools_dir: PathBuf,
    #[cfg(unix)]
    pub proton: crate::compat::Proton,
}

impl Setup {
    pub fn game_exe(&self) -> PathBuf {
        self.game_dir.join("win64").join("game.exe")
    }
}

/// `<library>` for a game folder at `<library>/steamapps/common/<name>`.
fn library_of(game_dir: &Path) -> Option<PathBuf> {
    let common = game_dir.parent()?;
    let steamapps = common.parent()?;
    let named = |p: &Path, n: &str| p.file_name().is_some_and(|f| f.eq_ignore_ascii_case(n));
    (named(common, "common") && named(steamapps, "steamapps")).then(|| steamapps.parent().map(Path::to_path_buf))?
}

/// The game install folder alone: `--game-dir`, `STAR_FORGE_GAME_DIR`, or Steam.
pub fn find_game(opts: &Options) -> Result<PathBuf, String> {
    let explicit = opts.game_dir.clone().or_else(|| crate::env::path(crate::env::GAME_DIR));
    let game_dir = match explicit {
        Some(d) => d,
        None => steam::find_app(&steam::libraries(&steam::steam_roots()), steam::GAME_APPID)
            .map(|(_, dir)| dir)
            .ok_or("Star Conflict was not found in any Steam library; pass --game-dir or set STAR_FORGE_GAME_DIR")?,
    };
    if !steam::is_game_dir(&game_dir) {
        return Err(format!("{} does not look like a Star Conflict install", game_dir.display()));
    }
    Ok(game_dir)
}

pub fn resolve(opts: &Options) -> Result<Setup, String> {
    let env = |explicit: &Option<PathBuf>, name| explicit.clone().or_else(|| crate::env::path(name));
    let opts = &Options {
        game_dir: env(&opts.game_dir, crate::env::GAME_DIR),
        tools_dir: env(&opts.tools_dir, crate::env::TOOLS_DIR),
        proton: env(&opts.proton, crate::env::PROTON),
    };
    let roots = steam::steam_roots();
    let libraries = steam::libraries(&roots);
    let game_dir = find_game(opts)?;

    let tools_dir = match &opts.tools_dir {
        Some(d) => d.clone(),
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("cannot find this program's folder")?
            .to_path_buf(),
    };
    for f in [INJECT_EXE, AGENT_DLL] {
        if !tools_dir.join(f).is_file() {
            return Err(format!("{f} is missing from {} (pass --tools-dir or set STAR_FORGE_TOOLS_DIR)", tools_dir.display()));
        }
    }

    #[cfg(unix)]
    let proton = {
        let library = library_of(&game_dir)
            .ok_or("the game folder is not inside a Steam library, so its Proton prefix is unknown")?;
        let steam_root = roots.first().cloned().unwrap_or_else(|| library.clone());
        crate::compat::detect(&steam_root, &library, &libraries, opts.proton.as_deref())?
    };
    #[cfg(windows)]
    let _ = (&opts.proton, library_of, &libraries);

    Ok(Setup {
        game_dir,
        tools_dir,
        #[cfg(unix)]
        proton,
    })
}

/// Path as the injector (a Windows program) should see it.
pub fn tool_path(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{} is not valid Unicode", path.display()))?;
    #[cfg(windows)]
    return Ok(text.to_string());
    #[cfg(unix)]
    {
        let ok = path.is_absolute()
            && !text.contains([':', '\\', '\0'])
            && path.components().all(|c| !matches!(c, Component::CurDir | Component::ParentDir));
        if !ok {
            return Err(format!("{text}: Wine needs an absolute path without ':' or '\\'"));
        }
        // Wine's Z: drive maps the Unix root.
        Ok(format!("Z:{}", text.replace('/', "\\")))
    }
}

/// Command that runs sc-inject.exe for the profile in `profile_dir`.
pub fn command(setup: &Setup, profile_dir: &Path, game_args: &[String]) -> Result<Command, String> {
    let profile_dir = std::fs::canonicalize(profile_dir)
        .map_err(|e| format!("{}: {e}", profile_dir.display()))?;
    #[cfg(windows)]
    let profile_dir = PathBuf::from(profile_dir.to_string_lossy().trim_start_matches(r"\\?\"));

    let inject = setup.tools_dir.join(INJECT_EXE);
    let mut args = vec![
        "--game".to_string(),
        tool_path(&setup.game_exe())?,
        "--profile".to_string(),
        tool_path(&profile_dir)?,
        "--dll".to_string(),
        tool_path(&setup.tools_dir.join(AGENT_DLL))?,
    ];
    if !game_args.is_empty() {
        args.push("--".to_string());
        args.extend(game_args.iter().cloned());
    }

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new(&inject);
        c.args(&args);
        c
    };

    #[cfg(unix)]
    let mut cmd = {
        let p = &setup.proton;
        let proton_script = p.dir.join("proton");
        let mut c = match &p.runtime {
            Some(run) => {
                let mut c = Command::new(run);
                c.arg("--").arg(&proton_script);
                // pressure-vessel only shares a fixed set of host paths read-write.
                let shared = [&p.compat_data, &setup.game_dir, &profile_dir, &setup.tools_dir];
                let mut rw: Vec<String> = Vec::new();
                for d in shared {
                    let s = d.to_str().ok_or("path is not valid Unicode")?;
                    if s.contains(':') {
                        return Err(format!("{s}: paths shared with Proton cannot contain ':'"));
                    }
                    rw.push(s.to_string());
                }
                let mut value = std::env::var("PRESSURE_VESSEL_FILESYSTEMS_RW").unwrap_or_default();
                for s in rw {
                    if !value.is_empty() {
                        value.push(':');
                    }
                    value.push_str(&s);
                }
                c.env("PRESSURE_VESSEL_FILESYSTEMS_RW", value);
                c
            }
            None => Command::new(&proton_script),
        };
        c.arg("waitforexitandrun").arg(&inject).args(&args);
        c.env("STEAM_COMPAT_DATA_PATH", &p.compat_data)
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &p.steam_root)
            .env("STEAM_COMPAT_APP_ID", steam::GAME_APPID)
            .env("SteamAppId", steam::GAME_APPID)
            .env("SteamGameId", steam::GAME_APPID);
        add_overlay(&mut c, &p.steam_root);
        c
    };

    crate::env::use_host_env(&mut cmd);
    cmd.current_dir(setup.game_dir.join("win64"));
    Ok(cmd)
}

/// Sets up the Steam overlay the way Steam does for the games it starts: its
/// renderer is preloaded and its Vulkan layer (which draws over DXVK) enabled.
#[cfg(unix)]
fn add_overlay(cmd: &mut Command, steam_root: &Path) {
    let libs: Vec<PathBuf> = ["ubuntu12_32", "ubuntu12_64"]
        .iter()
        .map(|d| steam_root.join(d).join("gameoverlayrenderer.so"))
        .filter(|p| p.is_file())
        .collect();
    if libs.is_empty() {
        return;
    }
    let mut preload = std::env::var("LD_PRELOAD").unwrap_or_default();
    for lib in libs {
        let Some(s) = lib.to_str() else { continue };
        if s.contains([':', ' ']) || preload.split([':', ' ']).any(|e| e == s) {
            continue;
        }
        if !preload.is_empty() {
            preload.push(':');
        }
        preload.push_str(s);
    }
    cmd.env("LD_PRELOAD", preload)
        .env("ENABLE_VK_LAYER_VALVE_steam_overlay_1", "1")
        .env("SteamOverlayGameId", steam::GAME_APPID);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_from_game_dir() {
        assert_eq!(
            library_of(Path::new("/s/lib/steamapps/common/star conflict")),
            Some(PathBuf::from("/s/lib"))
        );
        assert_eq!(library_of(Path::new("/games/star conflict")), None);
    }

    #[cfg(unix)]
    #[test]
    fn wine_paths() {
        assert_eq!(tool_path(Path::new("/home/user/star conflict")).unwrap(), r"Z:\home\user\star conflict");
        assert!(tool_path(Path::new("relative")).is_err());
        assert!(tool_path(Path::new("/a/../b")).is_err());
        assert!(tool_path(Path::new("/a:b")).is_err());
    }
}
