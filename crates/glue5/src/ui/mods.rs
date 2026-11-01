//! Mods: switched on or off here; the mods in the user directory's `mods`
//! folder, started with the game or during it; their menus while it runs;
//! GLUE 5 Studio (the web IDE the app serves) with its address, QR code
//! and pairing code; the mods' log.

use egui::{Color32, CornerRadius, RichText, Sense, vec2};
use glue5_mods::ModState;

use super::pages::{card_title, sheet, small};
use super::widgets::{Control, action_button, action_buttons, choice, inline_button, segmented, setting, slider, toggle};
use super::{Status, WARN, theme};
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    Menus,
    #[default]
    Mods,
    Studio,
    Log,
}

/// What the page keeps between frames.
#[derive(Default)]
pub struct ModsPage {
    tab: Tab,
    confirm_remove: Option<String>,
    dlc_scan: Option<glue5_mods::dlc::DlcScanReport>,
    pc_import_status: Option<Result<String, String>>,
}

impl ModsPage {
    /// Opens on the mods' menus.
    pub fn show_menus(&mut self) {
        self.tab = Tab::Menus;
    }
}

/// The Studio as it serves, for the page.
pub struct StudioInfo {
    /// The page's address, the pairing code in it.
    pub url: String,
    /// The pairing code as people type it.
    pub code: String,
    pub lan: bool,
}

const ERR: Color32 = Color32::from_rgb(240, 149, 149);

/// The mods in a line, for Settings: off, on, what runs.
pub(super) fn summary(settings: &Settings, status: &Status) -> String {
    match status.started.as_ref() {
        Some(s) if s.mods != settings.mods => {
            return if settings.mods { "On from the next start".into() } else { "Off from the next start".into() };
        }
        _ => {}
    }
    if !settings.mods {
        return "Off · scripts that change the game".into();
    }
    let mods = glue5_mods::mods();
    let mut parts = vec!["On".to_owned()];
    match mods.len() {
        0 => {}
        1 => parts.push("1 mod".into()),
        n => parts.push(format!("{n} mods")),
    }
    let running = mods.iter().filter(|m| m.state == ModState::Running).count();
    if running > 0 {
        parts.push(format!("{running} running"));
    }
    if status.studio.is_some() {
        parts.push("Studio on".into());
    }
    parts.join(" · ")
}

/// Android's Java document picker drops one selected file into the private inbox.
#[cfg(target_os = "android")]
fn process_android_pc_import(page: &mut ModsPage) {
    // Keep the inbox file until the Mods directory has been opened by the app.
    if glue5_mods::dir().is_none() { return; }
    use std::path::PathBuf;
    let Some(root) = std::env::var_os("GLUE5_APP_FILES_DIR").map(PathBuf::from) else { return };
    let inbox = root.join("pc-mod-import-inbox");
    for (file, kind) in [("incoming.json", "json"), ("incoming.zip", "zip"), ("incoming.ymap", "ymap"), ("incoming.rpf", "rpf")] {
        let path = inbox.join(file);
        if !path.is_file() { continue; }
        let result = (|| -> Result<String, String> {
            let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            let limit = match kind {
                "json" => 4 * 1024 * 1024,
                "zip" => 256 * 1024 * 1024,
                _ => 128 * 1024 * 1024,
            };
            if metadata.len() > limit {
                return Err(format!("Selected {kind} exceeds {} MiB limit", limit / (1024 * 1024)));
            }
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?.as_secs();
            if kind == "json" {
                let name = format!("map_{stamp}");
                let count = glue5_mods::install_placement_mod(&name, &bytes)?;
                Ok(format!("Imported {name}: {count} placements"))
            } else if kind == "ymap" {
                let name = format!("ymap_{stamp}");
                let report = glue5_mods::install_ymap_mod(&name, &bytes)?;
                Ok(format!("Imported {name}: {} HD placements; skipped {} MLO, {} LOD", report.supported_entities, report.skipped_mlo, report.skipped_lod))
            } else if kind == "rpf" {
                let name = format!("rpf_map_{stamp}");
                let report = glue5_mods::install_rpf_ymaps(&name, &bytes)?;
                for warning in &report.warnings {
                    log::warn!("RPF map import: {warning}");
                }
                Ok(format!("Imported {name}: {} of {} YMAPs, {} object placements; skipped {} maps; {} warnings", report.imported_maps, report.maps_found, report.entities, report.skipped_maps, report.warnings.len()))
            } else {
                let name = format!("pc_mod_{stamp}");
                // Inspect native vehicle-pack structure before installing the
                // ZIP. This is a compatibility report, not asset registration.
                let vehicle = glue5_mods::vehicle_packs::inspect_zip(&bytes)?;
                let report = glue5_mods::install_pc_package(&name, &bytes)?;
                let mut message = format!(
                    "Imported {name}: {:?}, {} files. {}",
                    report.kind, report.entries, vehicle.summary()
                );
                // Validate the imported DLC archives immediately. This parses
                // readable embedded YFT/YTD resources but does not register or
                // mount them in the game's engine.
                match glue5_mods::scan_pc_dlc_packages() {
                    Ok(scan) => {
                        let packs = scan.packs.iter().filter(|pack| pack.package == name).collect::<Vec<_>>();
                        if packs.is_empty() {
                            message.push_str("; no dlcpacks/<name>/dlc.rpf found for engine loading");
                        }
                        for pack in packs {
                            message.push_str(&format!(
                                "; {}: parsed {}/{} YFT and {}/{} YTD; invalid {}; unverified {}",
                                pack.name,
                                pack.valid_yft_files, pack.yft_files,
                                pack.valid_ytd_files, pack.ytd_files,
                                pack.invalid_asset_files, pack.unverified_asset_files
                            ));
                        }
                        for warning in scan.warnings.iter().take(4) {
                            message.push_str("; ");
                            message.push_str(warning);
                        }
                    }
                    Err(error) => {
                        message.push_str("; DLC asset validation unavailable: ");
                        message.push_str(&error);
                    }
                }
                for note in &vehicle.notes {
                    message.push_str("; ");
                    message.push_str(note);
                }
                Ok(message)
            }
        })();
        match &result {
            Ok(message) => log::info!("PC mod importer: {message}"),
            Err(error) => log::warn!("PC mod importer: {error}"),
        }
        page.pc_import_status = Some(result);
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(not(target_os = "android"))]
fn process_android_pc_import(_page: &mut ModsPage) {}

fn android_pc_import_controls(ui: &mut egui::Ui, page: &mut ModsPage, settings: &mut Settings, game_running: bool) {
    panel(ui, |ui| {
        card_title(ui, "IMPORT PC MODS", None);
        small(ui, "Choose a PC mod ZIP, JSON placement map, YMAP, or readable RPF archive. RPF import converts supported embedded exterior YMAP placements into a script mod; it does not mount the archive or load custom vehicle assets.", theme::MUTED);
        small(ui, "YMAP conversion only places existing exterior HD models; MLO interiors, LOD chains, collision and custom assets are not loaded. Scan DLC packs only inspects metadata; it does not activate DLC or load custom YFT/YTD vehicles. Native ASI/DLL plugins are not executed.", theme::MUTED);
        if !settings.mods {
            small(ui, "Turn Mods on first; the change takes effect when the game starts.", WARN);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if inline_button(ui, "Import PC mod ZIP").clicked() {
                #[cfg(target_os = "android")]
                request_android_pc_import("zip");
                #[cfg(not(target_os = "android"))]
                log::info!("PC mod file picker is Android-only.");
            }
            if inline_button(ui, "Import JSON map").clicked() {
                #[cfg(target_os = "android")]
                request_android_pc_import("json");
                #[cfg(not(target_os = "android"))]
                log::info!("JSON file picker is Android-only.");
            }
            if inline_button(ui, "Import YMAP").clicked() {
                #[cfg(target_os = "android")]
                request_android_pc_import("ymap");
                #[cfg(not(target_os = "android"))]
                log::info!("YMAP file picker is Android-only.");
            }
            if inline_button(ui, "Import RPF maps").clicked() {
                #[cfg(target_os = "android")]
                request_android_pc_import("rpf");
                #[cfg(not(target_os = "android"))]
                log::info!("PC mod file picker is Android-only.");
            }
            if inline_button(ui, "Scan DLC packs").clicked() {
                match glue5_mods::scan_pc_dlc_packages() {
                    Ok(report) => {
                        log::info!("DLC scan: {} packs found, {} registration entries, {} warnings",
                            report.packs.len(), report.registered_names.len(), report.warnings.len());
                        page.dlc_scan = Some(report);
                    }
                    Err(error) => log::warn!("DLC scan failed: {error}"),
                }
            }
        });
        if let Some(result) = &page.pc_import_status {
            ui.add_space(6.0 * theme::density(ui));
            small(ui, "LAST IMPORT RESULT", theme::MUTED);
            match result {
                Ok(message) => small(ui, message, theme::MUTED),
                Err(error) => small(ui, &format!("Import failed: {error}"), WARN),
            }
        }
        if let Some(report) = &page.dlc_scan {
            ui.add_space(6.0 * theme::density(ui));
            small(ui, "DLC SCAN · IMPORTED PACKAGES ONLY", theme::MUTED);
            if report.packs.is_empty() {
                small(ui, "No dlcpacks/<name>/dlc.rpf archives were found in imported packages.", theme::MUTED);
            }
            for pack in &report.packs {
                let registration = if pack.registered_in_imported_dlclist { "listed in imported dlclist" } else { "not listed in imported dlclist" };
                let order = pack.setup_order.map(|value| format!("order {value}")).unwrap_or_else(|| "order unavailable".to_owned());
                small(ui, &format!("{} · {} · {} · {} YFT / {} YTD · parsed {} · invalid {} · unverified {}", pack.name, order, registration, pack.yft_files, pack.ytd_files, pack.valid_yft_files + pack.valid_ytd_files, pack.invalid_asset_files, pack.unverified_asset_files), theme::MUTED);
            }
            for warning in report.warnings.iter().take(8) {
                small(ui, warning, WARN);
            }
            if report.warnings.len() > 8 {
                small(ui, &format!("{} more scan warnings", report.warnings.len() - 8), theme::MUTED);
            }
        }
        match glue5_mods::installed_pc_packages() {
            Ok(packages) if !packages.is_empty() => {
                ui.add_space(6.0 * theme::density(ui));
                small(ui, "INSTALLED PC PACKAGES · applied at next game launch", theme::MUTED);
                for package in packages {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&package.name).size(13.0 * theme::density(ui)).color(Color32::WHITE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{} files · {:.1} MiB", package.files, package.unpacked_bytes as f64 / 1_048_576.0)).size(11.0 * theme::density(ui)).color(theme::MUTED));
                            if game_running {
                                ui.add_enabled(false, egui::Button::new("Remove"));
                            } else if inline_button(ui, "Remove").clicked() {
                                match glue5_mods::uninstall_pc_package(&package.name) {
                                    Ok(()) => log::info!("PC package {} removed; it will no longer mount next launch.", package.name),
                                    Err(error) => log::warn!("PC package {} could not be removed: {error}", package.name),
                                }
                            }
                        });
                    });
                }
                if game_running {
                    small(ui, "Stop the game before removing an imported package.", theme::MUTED);
                }
            }
            Ok(_) => small(ui, "No PC ZIP packages imported yet.", theme::MUTED),
            Err(error) => small(ui, &format!("Package list: {error}"), WARN),
        }
    });
}

#[cfg(target_os = "android")]
fn request_android_pc_import(kind: &str) {
    let Some(root) = std::env::var_os("GLUE5_APP_FILES_DIR") else { return };
    let path = std::path::PathBuf::from(root).join("pc-mod-import-request");
    if let Err(error) = std::fs::write(path, kind) {
        log::warn!("PC mod picker request: {error}");
    }
}

/// True when closed.
pub(super) fn mods_page(ctx: &egui::Context, page: &mut ModsPage, settings: &mut Settings, status: &Status) -> bool {
    // Android's Java picker writes into the private inbox; consume it on the UI thread.
    process_android_pc_import(page);
    // Mods change by themselves (they end, fail, the Studio starts them).
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
    sheet(ctx, "mods-window", "MODS", |ui| {
        let scale = theme::density(ui);
        let running_with = status.started.as_ref().map(|s| s.mods);
        let note = match running_with {
            Some(with) if with != settings.mods => {
                Some(if settings.mods { "On from the next start of the game" } else { "Off from the next start; running mods keep going until then" })
            }
            _ => None,
        };
        setting(ui, "Mods", Control::Toggle, Some(note.unwrap_or("Luau scripts that change the game, run with it")), |ui| {
            toggle(ui, &mut settings.mods);
        });
        ui.add_space(6.0 * theme::density(ui));
        android_pc_import_controls(ui, page, settings, status.started.is_some());
        if !settings.mods && !status.mods {
            ui.add_space(4.0 * scale);
            panel(ui, |ui| {
                card_title(ui, "WHAT MODS ARE", None);
                small(
                    ui,
                    "A mod is a folder of Luau scripts that call the game's own functions: spawn a car, change \
                     the weather, add a menu of its own. Turn mods on, then write them in GLUE 5 Studio from a \
                     browser on your computer or tablet, or let Claude Code write them.",
                    theme::MUTED,
                );
            });
            return;
        }
        ui.add_space(6.0 * scale);
        // The menus are there while the game runs with mods.
        let mut tabs = vec![(Tab::Mods, "MODS"), (Tab::Studio, "STUDIO"), (Tab::Log, "LOG")];
        if status.mods {
            tabs.insert(0, (Tab::Menus, "MENUS"));
        } else if page.tab == Tab::Menus {
            page.tab = Tab::Mods;
        }
        segmented(ui, &mut page.tab, &tabs, 40.0 * scale);
        ui.add_space(10.0 * scale);
        match page.tab {
            Tab::Menus => menus_tab(ui),
            Tab::Mods => mods_tab(ui, page, status),
            Tab::Studio => studio_tab(ui, settings, status),
            Tab::Log => log_tab(ui),
        }
    })
}

/// The mods' menus, each under its title: their items with the app's own
/// widgets; what the player does goes to the mod.
fn menus_tab(ui: &mut egui::Ui) {
    let scale = theme::density(ui);
    let menus = glue5_mods::menus();
    if menus.is_empty() {
        muted(ui, "No menus. A mod makes one with ui.menu(\"Title\"); F7 opens them during the game.");
        return;
    }
    for menu in &menus {
        ui.add_space(4.0 * scale);
        ui.horizontal(|ui| {
            ui.label(theme::heading(&menu.title.to_uppercase(), 17.0 * scale).color(Color32::WHITE));
            ui.label(RichText::new(&menu.module).size(12.0 * scale).color(theme::MUTED));
        });
        ui.push_id(menu.id, |ui| {
            for item in &menu.items {
                ui.push_id(item.id, |ui| menu_item(ui, &menu.module, item));
            }
        });
    }
}

fn menu_item(ui: &mut egui::Ui, module: &str, item: &glue5_mods::MenuItem) {
    use glue5_mods::{MenuInput, MenuKind};
    let note = item.note.as_deref();
    match &item.kind {
        MenuKind::Toggle(on) => {
            let mut v = *on;
            setting(ui, &item.label, Control::Toggle, note, |ui| {
                if toggle(ui, &mut v).changed() {
                    glue5_mods::menu_input(module, item.id, MenuInput::Toggle(v));
                }
            });
        }
        MenuKind::Slider { value, min, max, step } => {
            let mut v = *value as f32;
            let decimals = if step.fract() == 0.0 { 0 } else { 2 };
            setting(ui, &item.label, Control::Slider, note, |ui| {
                if slider(ui, &mut v, *min as f32..=*max as f32, *step, "", decimals).changed() {
                    glue5_mods::menu_input(module, item.id, MenuInput::Value(f64::from(v)));
                }
            });
        }
        MenuKind::Choice { options, index } => {
            let mut i = *index;
            let labelled: Vec<(usize, &str)> = options.iter().enumerate().map(|(n, o)| (n, o.as_str())).collect();
            setting(ui, &item.label, Control::Choice, note, |ui| {
                if choice(ui, &mut i, &labelled).changed() {
                    glue5_mods::menu_input(module, item.id, MenuInput::Choose(i));
                }
            });
        }
        MenuKind::Button => {
            setting(ui, &item.label, Control::Action, note, |ui| {
                if action_button(ui, "Run").clicked() {
                    glue5_mods::menu_input(module, item.id, MenuInput::Press);
                }
            });
        }
        MenuKind::Label => muted(ui, &item.label),
    }
}

fn mods_tab(ui: &mut egui::Ui, page: &mut ModsPage, status: &Status) {
    let held = glue5_mods::held_back();
    if !held.is_empty() {
        small(
            ui,
            &format!("{} did not start with the game: it was running when the game stopped last time.", held.join(", ")),
            WARN,
        );
    }
    let mods = glue5_mods::mods();
    if mods.is_empty() {
        panel(ui, |ui| {
            card_title(ui, "NO MODS YET", None);
            small(ui, "Write one in GLUE 5 Studio, from a browser on this network.", theme::MUTED);
            if let Some(dir) = glue5_mods::dir().filter(|_| !crate::paths::MOBILE) {
                small(ui, &format!("Or put a folder with a main.luau in {}", dir.display()), theme::MUTED);
            }
            ui.add_space(4.0 * theme::density(ui));
            if inline_button(ui, "Open Studio").clicked() {
                page.tab = Tab::Studio;
            }
        });
        return;
    }
    // Before the game: which start with it.
    if !status.mods {
        for m in &mods {
            let mut on = m.autostart;
            let note = if on { "Enabled · starts with the game" } else { "Disabled · does not start with the game" };
            setting(ui, &m.name, Control::Toggle, Some(note), |ui| {
                if toggle(ui, &mut on).changed()
                    && let Err(e) = glue5_mods::set_autostart(&m.name, on)
                {
                    log::warn!("mods: {e}");
                }
            });
            if page.confirm_remove.as_deref() == Some(m.name.as_str()) {
                setting(ui, &format!("Uninstall {}?", m.name), Control::Action, Some("This permanently removes this mod's files"), |ui| {
                    let buttons = action_buttons(ui, &["Confirm", "Cancel"]);
                    if buttons[0].clicked() {
                        match glue5_mods::uninstall(&m.name) {
                            Ok(()) => page.confirm_remove = None,
                            Err(e) => log::warn!("mods: {e}"),
                        }
                    }
                    if buttons[1].clicked() {
                        page.confirm_remove = None;
                    }
                });
            } else {
                setting(ui, &format!("Remove {}", m.name), Control::Action, Some("Uninstall this mod and its files"), |ui| {
                    if action_button(ui, "Uninstall").clicked() {
                        page.confirm_remove = Some(m.name.clone());
                    }
                });
            }
        }
        return;
    }
    for m in &mods {
        let subtitle = match m.state {
            ModState::Running if m.waiting => "Waiting".to_owned(),
            ModState::Running => format!("Running · {:.1} ms per frame", m.cpu_ms),
            ModState::Error => format!("Error · {}", m.error.as_deref().unwrap_or("")),
            ModState::Stopped if m.autostart => "Stopped · starts with the game".to_owned(),
            ModState::Stopped => "Stopped".to_owned(),
        };
        let dot = match m.state {
            ModState::Running => "• ",
            _ => "",
        };
        setting(ui, &format!("{dot}{}", m.name), Control::Action, Some(&subtitle), |ui| {
            let running = m.state == ModState::Running;
            if action_button(ui, if running { "Stop" } else { "Start" }).clicked() {
                if running {
                    glue5_mods::stop(&m.name);
                } else {
                    glue5_mods::start(&m.name);
                }
            }
        });
        if page.confirm_remove.as_deref() == Some(m.name.as_str()) {
            setting(ui, &format!("Uninstall {}?", m.name), Control::Action, Some("This permanently removes this mod's files"), |ui| {
                let buttons = action_buttons(ui, &["Confirm", "Cancel"]);
                if buttons[0].clicked() {
                    match glue5_mods::uninstall(&m.name) {
                        Ok(()) => page.confirm_remove = None,
                        Err(e) => log::warn!("mods: {e}"),
                    }
                }
                if buttons[1].clicked() {
                    page.confirm_remove = None;
                }
            });
        } else {
            setting(ui, &format!("Remove {}", m.name), Control::Action, Some("Uninstall this mod and its files"), |ui| {
                if action_button(ui, "Uninstall").clicked() {
                    if m.state == ModState::Running {
                        glue5_mods::stop(&m.name);
                        log::warn!("mods: stop {} and try uninstall again", m.name);
                    } else {
                        page.confirm_remove = Some(m.name.clone());
                    }
                }
            });
        }
    }
    let running: Vec<_> = mods.iter().filter(|m| m.state == ModState::Running).collect();
    if !running.is_empty() {
        let total: f32 = running.iter().map(|m| m.cpu_ms).sum();
        setting(ui, &format!("{} running", running.len()), Control::Action, Some(&format!("{total:.1} ms per frame")), |ui| {
            if action_button(ui, "Stop all").clicked() {
                for m in &running {
                    glue5_mods::stop(&m.name);
                }
            }
        });
    }
    if glue5_mods::tainted() {
        small(ui, "A native failed inside the game: restart the game before long.", WARN);
    }
}

fn studio_tab(ui: &mut egui::Ui, settings: &mut Settings, status: &Status) {
    setting(ui, "GLUE 5 Studio", Control::Toggle, Some("Write and run mods from a browser"), |ui| {
        toggle(ui, &mut settings.studio);
    });
    let others = if settings.studio_lan { "Browsers on this network connect with the code" } else { "Only a browser on this device" };
    setting(ui, "Other devices", Control::Toggle, Some(others), |ui| {
        toggle(ui, &mut settings.studio_lan);
    });
    match &status.studio {
        Some(info) => {
            // The address as typed; copied with the code in it.
            let address = info.url.split('?').next().unwrap_or(&info.url).trim_end_matches('/');
            let open = if info.lan { "Open it in a browser on this network" } else { "Open it in a browser on this device" };
            setting(ui, address, Control::Action, Some(open), |ui| {
                if action_button(ui, "Copy").clicked() {
                    ui.ctx().copy_text(info.url.clone());
                }
            });
            setting(ui, &info.code, Control::Action, Some("Pairing code: the page asks for it once"), |ui| {
                if action_button(ui, "Copy").clicked() {
                    ui.ctx().copy_text(info.code.clone());
                }
            });
            if info.lan {
                qr_row(ui, &info.url);
            }
        }
        None if settings.studio => small(ui, "Starting…", theme::MUTED),
        None => {}
    }
    muted(ui, "Whoever has the code can run code in the game; it changes each time the Studio starts.");
}

/// The address as a QR code, in a row like the settings': a tablet's or a
/// phone's camera opens the Studio, paired.
fn qr_row(ui: &mut egui::Ui, url: &str) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(Color32::from_gray(28))
        .corner_radius((14.0 * scale) as u8)
        .inner_margin((12.0 * scale) as i8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                qr(ui, url, 112.0 * scale);
                ui.add_space(8.0 * scale);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Scan to open").size(14.0 * scale).color(Color32::WHITE));
                    ui.label(
                        RichText::new("With a tablet's or a phone's camera: the Studio opens already paired.")
                            .size(11.0 * scale)
                            .color(theme::MUTED),
                    );
                });
            });
        });
}

fn log_tab(ui: &mut egui::Ui) {
    let scale = theme::density(ui);
    let (lines, _) = glue5_mods::logs(0);
    if lines.is_empty() {
        muted(ui, "Nothing yet.");
    }
    panel(ui, |ui| {
        for l in lines.iter().rev().take(80).rev() {
            let color = if l.level == glue5_mods::LogLevel::Error { ERR } else { Color32::from_gray(213) };
            ui.label(RichText::new(format!("{} · {}", l.module, l.text)).monospace().size(12.0 * scale).color(color));
        }
    });
}

/// `text` as a QR code, dark on white.
fn qr(ui: &mut egui::Ui, text: &str, size: f32) {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else { return };
    let modules = code.width();
    let colors = code.to_colors();
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same((10.0 * theme::density(ui)) as u8), Color32::WHITE);
    let quiet = 2.0;
    let cell = size / (modules as f32 + 2.0 * quiet);
    for (i, c) in colors.iter().enumerate() {
        if *c == qrcode::Color::Dark {
            let (x, y) = ((i % modules) as f32, (i / modules) as f32);
            let min = rect.min + vec2((x + quiet) * cell, (y + quiet) * cell);
            painter.rect_filled(egui::Rect::from_min_size(min, vec2(cell + 0.5, cell + 0.5)), 0, theme::INK);
        }
    }
}

fn panel(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let scale = theme::density(ui);
    egui::Frame::new()
        .fill(Color32::from_gray(28))
        .corner_radius((14.0 * scale) as u8)
        .inner_margin((14.0 * scale) as i8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0 * scale;
            add(ui);
        });
}

fn muted(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0 * theme::density(ui)).color(theme::MUTED));
}
