//! Bundled, Android-ready Glue Trainer files. Installed into the user mods directory
//! without overwriting existing files, so the user's other mods and edits are preserved.

use std::path::Path;

const FILES: &[(&str, &str)] = &[
    ("main.luau", include_str!("../../../mods/glue-trainer/main.luau")),
    ("mod.json", include_str!("../../../mods/glue-trainer/mod.json")),
    ("core/bind.luau", include_str!("../../../mods/glue-trainer/core/bind.luau")),
    ("core/catalog.luau", include_str!("../../../mods/glue-trainer/core/catalog.luau")),
    ("core/game.luau", include_str!("../../../mods/glue-trainer/core/game.luau")),
    ("core/keys.luau", include_str!("../../../mods/glue-trainer/core/keys.luau")),
    ("core/lists.luau", include_str!("../../../mods/glue-trainer/core/lists.luau")),
    ("core/loop.luau", include_str!("../../../mods/glue-trainer/core/loop.luau")),
    ("core/prefs.luau", include_str!("../../../mods/glue-trainer/core/prefs.luau")),
    ("data/blips.luau", include_str!("../../../mods/glue-trainer/data/blips.luau")),
    ("data/hud.luau", include_str!("../../../mods/glue-trainer/data/hud.luau")),
    ("data/interiors.luau", include_str!("../../../mods/glue-trainer/data/interiors.luau")),
    ("data/looks.luau", include_str!("../../../mods/glue-trainer/data/looks.luau")),
    ("data/peds.luau", include_str!("../../../mods/glue-trainer/data/peds.luau")),
    ("data/places.luau", include_str!("../../../mods/glue-trainer/data/places.luau")),
    ("data/teleports.luau", include_str!("../../../mods/glue-trainer/data/teleports.luau")),
    ("data/vehicles.luau", include_str!("../../../mods/glue-trainer/data/vehicles.luau")),
    ("data/world.luau", include_str!("../../../mods/glue-trainer/data/world.luau")),
    ("features/customs.luau", include_str!("../../../mods/glue-trainer/features/customs.luau")),
    ("features/hub.luau", include_str!("../../../mods/glue-trainer/features/hub.luau")),
    ("features/looks.luau", include_str!("../../../mods/glue-trainer/features/looks.luau")),
    ("features/misc.luau", include_str!("../../../mods/glue-trainer/features/misc.luau")),
    ("features/movement.luau", include_str!("../../../mods/glue-trainer/features/movement.luau")),
    ("features/overlay.luau", include_str!("../../../mods/glue-trainer/features/overlay.luau")),
    ("features/players.luau", include_str!("../../../mods/glue-trainer/features/players.luau")),
    ("features/self.luau", include_str!("../../../mods/glue-trainer/features/self.luau")),
    ("features/settings.luau", include_str!("../../../mods/glue-trainer/features/settings.luau")),
    ("features/spawner.luau", include_str!("../../../mods/glue-trainer/features/spawner.luau")),
    ("features/teleport.luau", include_str!("../../../mods/glue-trainer/features/teleport.luau")),
    ("features/vehicle.luau", include_str!("../../../mods/glue-trainer/features/vehicle.luau")),
    ("features/world.luau", include_str!("../../../mods/glue-trainer/features/world.luau")),
];

pub fn install(mods_dir: &Path) {
    let root = mods_dir.join("glue-trainer");
    for (relative, contents) in FILES {
        let path = root.join(relative);
        // Refresh the trainer entry point and car customizer on upgrade.
        // Older APKs may already have seeded these files into persistent user storage;
        // leaving them untouched would keep the old menu forever after installing a new APK.
        if path.exists() && relative != "main.luau" && relative != "features/customs.luau" {
            continue;
        }
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                log::warn!("mods: cannot create Glue Trainer directory {}: {error}", parent.display());
                continue;
            }
        }
        if let Err(error) = std::fs::write(&path, contents) {
            log::warn!("mods: cannot install Glue Trainer file {}: {error}", path.display());
        }
    }
}
