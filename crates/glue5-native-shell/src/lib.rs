//! WASM-independent shell state and UI for native-engine frontends.
//!
//! This crate deliberately does not launch an engine or declare JNI symbols.
//! A platform adapter must implement NativeEngine using the exact host contract
//! supplied by the selected engine build before gameplay can be started.

use egui::{CentralPanel, Context, RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Home,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineState {
    #[default]
    NotAttached,
    Ready,
    Starting,
    Running,
    Stopped,
    Failed,
}

/// Safe, serializable-in-spirit options passed to a verified native adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchOptions {
    pub max_fps: u32,
    pub render_scale: f32,
    pub audio_enabled: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            max_fps: 60,
            render_scale: 1.0,
            audio_enabled: true,
        }
    }
}

/// Narrow boundary between the reusable shell and a build-specific engine host.
/// Implementations must be supplied by a verified Android/SDL adapter.
pub trait NativeEngine {
    fn state(&self) -> EngineState;
    fn start(&mut self, options: &LaunchOptions) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct NativeShell {
    pub screen: Screen,
    pub options: LaunchOptions,
    pub last_error: Option<String>,
}

impl NativeShell {
    pub fn show(&mut self, ctx: &Context, engine: &mut impl NativeEngine) {
        CentralPanel::default().show(ctx, |ui| self.show_contents(ui, engine));
    }

    fn show_contents(&mut self, ui: &mut Ui, engine: &mut impl NativeEngine) {
        ui.heading(RichText::new("GLUE 5").strong());
        ui.label("Native engine mode");

        ui.horizontal(|ui| {
            if ui.selectable_label(self.screen == Screen::Home, "Home").clicked() {
                self.screen = Screen::Home;
            }
            if ui.selectable_label(self.screen == Screen::Settings, "Settings").clicked() {
                self.screen = Screen::Settings;
            }
        });
        ui.separator();

        match self.screen {
            Screen::Home => self.show_home(ui, engine),
            Screen::Settings => self.show_settings(ui),
        }

        if let Some(error) = &self.last_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }

    fn show_home(&mut self, ui: &mut Ui, engine: &mut impl NativeEngine) {
        ui.label(format!("Engine: {:?}", engine.state()));
        match engine.state() {
            EngineState::Ready | EngineState::Stopped => {
                if ui.button("Start game").clicked() {
                    self.last_error = engine.start(&self.options).err();
                }
            }
            EngineState::Running | EngineState::Starting => {
                if ui.button("Stop game").clicked() {
                    self.last_error = engine.stop().err();
                }
            }
            EngineState::NotAttached => {
                ui.label("Native engine adapter is not attached.");
            }
            EngineState::Failed => {
                ui.label("The native engine reported a failure.");
            }
        }
    }

    fn show_settings(&mut self, ui: &mut Ui) {
        ui.add(egui::Slider::new(&mut self.options.max_fps, 30..=120).text("FPS limit"));
        ui.add(
            egui::Slider::new(&mut self.options.render_scale, 0.5..=1.0)
                .text("Render scale"),
        );
        ui.checkbox(&mut self.options.audio_enabled, "Audio enabled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative_and_clear() {
        let options = LaunchOptions::default();
        assert_eq!(options.max_fps, 60);
        assert_eq!(options.render_scale, 1.0);
        assert!(options.audio_enabled);
    }

    #[test]
    fn shell_starts_on_home() {
        assert_eq!(NativeShell::default().screen, Screen::Home);
    }
}
