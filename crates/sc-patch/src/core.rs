//! Core patches: patchsets that ship with STAR FORGE (`core-patches/` in the
//! repository) for behaviour many packs need. A pack turns them on by id
//! (`"patches": {"core": [...]}` in pack.json) instead of carrying the bytes
//! itself; they are applied exactly like the pack's own patchsets.

use crate::Patchset;

/// The bundled patchsets, by file name.
const SETS: [(&str, &str); 1] = [("core/game.exe.patchset.toml", include_str!("../../../core-patches/game.exe.patchset.toml"))];

fn all() -> Vec<Patchset> {
    SETS.iter().map(|(name, text)| Patchset::parse(text, name).expect("bundled core patchset parses")).collect()
}

/// Every core patch id, for validation and documentation.
pub fn ids() -> Vec<String> {
    all().into_iter().flat_map(|s| s.patches.into_iter().map(|p| p.id)).collect()
}

/// The core patchsets holding just the requested patches. Unknown ids are an
/// error naming them.
pub fn sets(requested: &[String]) -> Result<Vec<Patchset>, String> {
    let known = ids();
    if let Some(bad) = requested.iter().find(|id| !known.contains(id)) {
        return Err(format!("unknown core patch {bad:?} (this launcher has: {})", known.join(", ")));
    }
    Ok(all()
        .into_iter()
        .filter_map(|mut set| {
            set.patches.retain(|p| requested.contains(&p.id));
            (!set.patches.is_empty()).then_some(set)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_sets_parse_and_filter() {
        assert!(ids().contains(&"loose-before-packed".to_string()));
        let sets = sets(&["loose-before-packed".into()]).unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].patches.len(), 1);
        assert!(crate::overlaps(&sets[0].patches.iter().collect::<Vec<_>>()).is_empty());
        assert!(super::sets(&[]).unwrap().is_empty());
        assert!(super::sets(&["nope".into()]).unwrap_err().contains("nope"));
    }
}
