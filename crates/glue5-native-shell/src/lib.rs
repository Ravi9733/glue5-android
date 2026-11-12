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
        Self { max_fps: 60, render_scale: 1.0, audio_enabled: true }
    }
}

impl LaunchOptions {
    /// Reject unsafe or nonsensical values before crossing the platform boundary.
    /// UI widgets constrain these values too, but adapters may be called directly.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(30..=120).contains(&self.max_fps) {
            return Err("FPS limit must be between 30 and 120");
        }
        if !self.render_scale.is_finite() || !(0.5..=1.0).contains(&self.render_scale) {
            return Err("Render scale must be between 0.5 and 1.0");
        }
        Ok(())
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
    /// Request a launch only from a state in which a fresh engine session is valid.
    /// Keeping this guard outside the egui click handler also protects other frontends.
    pub fn start_game(&mut self, engine: &mut impl NativeEngine) {
        if !matches!(engine.state(), EngineState::Ready | EngineState::Stopped) {
            self.last_error = Some(format!("Cannot start the native engine while it is {:?}.", engine.state()));
            return;
        }

        self.last_error =
            self.options.validate().map_err(str::to_owned).and_then(|()| engine.start(&self.options)).err();
    }

    /// Request shutdown only while an engine session is active or starting.
    pub fn stop_game(&mut self, engine: &mut impl NativeEngine) {
        if !matches!(engine.state(), EngineState::Running | EngineState::Starting) {
            self.last_error = Some(format!("Cannot stop the native engine while it is {:?}.", engine.state()));
            return;
        }

        self.last_error = engine.stop().err();
    }

    pub fn show(&mut self, ui: &mut Ui, engine: &mut impl NativeEngine) {
        CentralPanel::default().show_inside(ui, |ui| self.show_contents(ui, engine));
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
                    self.start_game(engine);
                }
            }
            EngineState::Running | EngineState::Starting => {
                if ui.button("Stop game").clicked() {
                    self.stop_game(engine);
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
        ui.add(egui::Slider::new(&mut self.options.render_scale, 0.5..=1.0).text("Render scale"));
        ui.checkbox(&mut self.options.audio_enabled, "Audio enabled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockEngine {
        state: EngineState,
        starts: usize,
        stops: usize,
    }

    impl NativeEngine for MockEngine {
        fn state(&self) -> EngineState {
            self.state
        }

        fn start(&mut self, _options: &LaunchOptions) -> Result<(), String> {
            self.starts += 1;
            self.state = EngineState::Starting;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), String> {
            self.stops += 1;
            self.state = EngineState::Stopped;
            Ok(())
        }
    }

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

    #[test]
    fn default_options_pass_validation() {
        assert!(LaunchOptions::default().validate().is_ok());
    }

    #[test]
    fn invalid_fps_is_rejected() {
        let options = LaunchOptions { max_fps: 144, ..Default::default() };
        assert_eq!(options.validate(), Err("FPS limit must be between 30 and 120"));
    }

    #[test]
    fn start_is_forwarded_only_from_ready_or_stopped() {
        let mut shell = NativeShell::default();
        let mut engine = MockEngine { state: EngineState::Ready, ..Default::default() };
        shell.start_game(&mut engine);
        assert_eq!(engine.starts, 1);
        assert_eq!(engine.state, EngineState::Starting);
    }

    #[test]
    fn start_is_rejected_when_engine_is_not_attached() {
        let mut shell = NativeShell::default();
        let mut engine = MockEngine::default();
        shell.start_game(&mut engine);
        assert_eq!(engine.starts, 0);
        assert!(shell.last_error.as_deref().unwrap().contains("NotAttached"));
    }

    #[test]
    fn stop_is_forwarded_only_for_active_engine() {
        let mut shell = NativeShell::default();
        let mut engine = MockEngine { state: EngineState::Running, ..Default::default() };
        shell.stop_game(&mut engine);
        assert_eq!(engine.stops, 1);
        assert_eq!(engine.state, EngineState::Stopped);
    }

    #[test]
    fn stop_is_rejected_when_engine_is_not_running() {
        let mut shell = NativeShell::default();
        let mut engine = MockEngine { state: EngineState::Ready, ..Default::default() };
        shell.stop_game(&mut engine);
        assert_eq!(engine.stops, 0);
        assert!(shell.last_error.as_deref().unwrap().contains("Ready"));
    }

    #[test]
    fn invalid_render_scale_is_rejected() {
        for render_scale in [0.0, 1.1, f32::NAN, f32::INFINITY] {
            let options = LaunchOptions { render_scale, ..Default::default() };
            assert_eq!(options.validate(), Err("Render scale must be between 0.5 and 1.0"));
        }
    }
}
