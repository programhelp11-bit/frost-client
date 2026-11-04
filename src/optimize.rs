//! Mod-free FPS boost: rewrites performance related keys in the instance's `options.txt` before launch.
//! Unknown keys are ignored by the game, so one set per format family is safe.

use std::path::Path;

const LEGACY: &[(&str, &str)] = &[
    ("fancyGraphics", "false"),
    ("ao", "0"),
    ("clouds", "false"),
    ("particles", "2"),
    ("mipmapLevels", "0"),
    ("enableVsync", "false"),
    ("maxFps", "260"),
    ("entityShadows", "false"),
    ("useVbo", "true"),
    ("fboEnable", "true"),
    ("viewBobbing", "false"),
];

const MODERN: &[(&str, &str)] = &[
    ("graphicsMode", "0"),
    ("fancyGraphics", "false"),
    ("ao", "false"),
    ("renderClouds", "\"false\""),
    ("particles", "2"),
    ("mipmapLevels", "0"),
    ("enableVsync", "false"),
    ("maxFps", "260"),
    ("entityShadows", "false"),
    ("biomeBlendRadius", "0"),
    ("entityDistanceScaling", "0.75"),
    ("menuBackgroundBlurriness", "0"),
    ("cutoutLeaves", "false"),
    ("improvedTransparency", "false"),
];

/// Keys that are only lowered, never raised (the player may prefer less).
const CAPS: &[(&str, i32)] = &[("renderDistance", 8), ("simulationDistance", 6)];

fn is_legacy(version_id: &str) -> bool {
    version_id.starts_with("1.8")
}

/// Pure transformation of an `options.txt` body (testable without touching the disk).
pub fn optimize_options(existing: &str, version_id: &str) -> String {
    let mut lines: Vec<String> = existing.lines().map(str::to_owned).collect();
    let set: &[(&str, &str)] = if is_legacy(version_id) { LEGACY } else { MODERN };
    let upsert = |lines: &mut Vec<String>, key: &str, value: String| {
        let prefix = format!("{key}:");
        match lines.iter_mut().find(|l| l.starts_with(&prefix)) {
            Some(l) => *l = format!("{key}:{value}"),
            None => lines.push(format!("{key}:{value}")),
        }
    };
    for (k, v) in set {
        upsert(&mut lines, k, (*v).to_owned());
    }
    for (k, cap) in CAPS {
        if *k == "simulationDistance" && is_legacy(version_id) {
            continue;
        }
        let prefix = format!("{k}:");
        let current = lines
            .iter()
            .find(|l| l.starts_with(&prefix))
            .and_then(|l| l[prefix.len()..].trim().parse::<i32>().ok());
        match current {
            Some(c) if c <= *cap => {}
            _ => upsert(&mut lines, k, cap.to_string()),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Applies the profile to `<game_dir>/options.txt` (created when missing). Errors are ignored by callers.
pub fn apply(game_dir: &Path, version_id: &str) -> std::io::Result<()> {
    let path = game_dir.join("options.txt");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, optimize_options(&existing, version_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_sets_graphics_and_keeps_other_keys() {
        let out = optimize_options("lang:hu_hu\nrenderDistance:16\nfancyGraphics:true\n", "1.21.10");
        assert!(out.contains("lang:hu_hu"));
        assert!(out.contains("renderDistance:8"));
        assert!(out.contains("graphicsMode:0"));
        assert!(out.contains("fancyGraphics:false"));
        assert!(out.contains("renderClouds:\"false\""));
        assert!(out.contains("enableVsync:false"));
    }

    #[test]
    fn lower_render_distance_is_kept() {
        let out = optimize_options("renderDistance:5\n", "1.21.11");
        assert!(out.contains("renderDistance:5"));
        assert!(!out.contains("renderDistance:8"));
    }

    #[test]
    fn legacy_uses_old_keys() {
        let out = optimize_options("", "1.8.9");
        assert!(out.contains("ao:0"));
        assert!(out.contains("clouds:false"));
        assert!(!out.contains("graphicsMode"));
        assert!(!out.contains("simulationDistance"));
        assert!(out.contains("renderDistance:8"));
    }
}
