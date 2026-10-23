//! Client patches from a profile's `patches/` folder, seen from the launcher:
//! a summary for the interface and a check against the installed game before
//! it starts, so a pack made for another game build fails here with a clear
//! message instead of inside the game.
//!
//! Applying them is the patch method's job; for `inject` that is the agent.

use sc_patch::Patchset;
use sc_profile::{OnError, Profile, PATCHES_DIR};
use std::path::Path;

/// Everything a profile patches: the core patches its pack turns on, then the
/// pack's own patchsets.
pub fn all_sets(profile_dir: &Path, profile: &Profile) -> Vec<Result<Patchset, String>> {
    let mut out: Vec<Result<Patchset, String>> = match sc_patch::core::sets(&profile.patches.core) {
        Ok(sets) => sets.into_iter().map(Ok).collect(),
        Err(e) => vec![Err(e)],
    };
    out.extend(load_sets(profile_dir));
    out
}

/// The patchsets in a profile folder, parsed. Unreadable ones are errors.
pub fn load_sets(profile_dir: &Path) -> Vec<Result<Patchset, String>> {
    sc_profile::patchset_files(profile_dir)
        .into_iter()
        .map(|file| {
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            Patchset::parse(&text, &format!("{PATCHES_DIR}/{name}")).map_err(|e| e.to_string())
        })
        .collect()
}

/// What a profile patches, for display.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Modules with a patchset, e.g. `game.exe`.
    pub modules: Vec<String>,
    /// Patches that are on by default (after the player's choices).
    pub patches: usize,
    pub plugins: usize,
    pub errors: Vec<String>,
}

pub fn summary(profile_dir: &Path, profile: &Profile) -> Summary {
    let mut s = Summary { plugins: profile.patches.plugins.len(), ..Summary::default() };
    for set in all_sets(profile_dir, profile) {
        match set {
            Ok(set) => {
                s.patches += set.enabled(choices(profile, &set.exe)).0.len();
                if !s.modules.contains(&set.exe) {
                    s.modules.push(set.exe);
                }
            }
            Err(e) => s.errors.push(e),
        }
    }
    s
}

fn choices<'a>(profile: &'a Profile, exe: &str) -> Option<&'a std::collections::HashMap<String, bool>> {
    profile.patch_choices.iter().find(|(k, _)| k.eq_ignore_ascii_case(exe)).map(|(_, v)| v)
}

/// The module file a patchset targets: next to game.exe, or under the game root.
fn module_file(win64: &Path, exe: &str) -> Option<std::path::PathBuf> {
    let find = |dir: &Path| {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(exe)))
    };
    find(win64).or_else(|| win64.parent().and_then(find))
}

/// Checks every enabled patch against the installed files. `Err` means the
/// game must not start (the pack's `on_error` is `exit`); the warnings are
/// worth showing either way.
pub fn preflight(profile_dir: &Path, profile: &Profile, game_exe: &Path) -> Result<Vec<String>, String> {
    let win64 = game_exe.parent().unwrap_or(Path::new("."));
    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    for set in all_sets(profile_dir, profile) {
        let set = match set {
            Ok(s) => s,
            Err(e) => {
                problems.push(e);
                continue;
            }
        };
        let Some(file) = module_file(win64, &set.exe) else {
            problems.push(format!("{} is not in the game folder", set.exe));
            continue;
        };
        match std::fs::read(&file) {
            Ok(data) => {
                let r = sc_patch::verify_file(&set, data, choices(profile, &set.exe));
                problems.extend(r.errors);
                warnings.extend(r.warnings);
                warnings.extend(r.skipped.into_iter().map(|s| format!("{}: optional patch skipped: {s}", set.exe)));
            }
            Err(e) => problems.push(format!("{}: {e}", file.display())),
        }
    }
    for plugin in &profile.patches.plugins {
        if !sc_profile::patches_rel(plugin).is_some_and(|rel| profile_dir.join(PATCHES_DIR).join(rel).is_file()) {
            problems.push(format!("plugin {plugin} is missing from the profile"));
        }
    }
    if problems.is_empty() {
        return Ok(warnings);
    }
    let text = format!("The pack's game patches don't match this game: {}", problems.join("; "));
    match profile.patches.on_error {
        OnError::Exit => Err(text),
        OnError::Continue => {
            warnings.insert(0, format!("{text}. Starting anyway (the pack allows it)."));
            Ok(warnings)
        }
    }
}
