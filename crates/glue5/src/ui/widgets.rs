//! Glue 5-specific text entry and compatibility exports for the UI kit.
use super::TextKind;
use super::theme::{MUTED, density};
use super::kit::focus;
use egui::*;

pub(super) use super::kit::{action_button, action_buttons, choice, close_button, segmented, setting, slider, toggle, Control, focus as focus_control, inline_button};

/// A line of text in a field of its own, its name on the left (if any): the
/// choice control's look. Anywhere on the field gives the text the focus.
/// `kind` picks the system keyboard on touch screens, where an empty code or
/// address field also offers to paste (their keyboard has no way to).
pub(super) fn text_field(ui: &mut Ui, label: &str, kind: TextKind, text: &mut String, hint: &str, max: usize) -> Response {
    let scale = density(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0 * scale), Sense::hover());
    let radius = 12.0 * scale;
    ui.painter().rect_filled(rect, radius, Color32::from_gray(36));
    let field = ui.interact(rect, ui.id().with(label), Sense::click());
    let mut inner = rect.shrink2(vec2(14.0 * scale, 0.0));
    let label_width = if label.is_empty() { 0.0 } else { 52.0 * scale };
    ui.painter().text(inner.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(13.0 * scale), MUTED);
    if crate::paths::TOUCH && matches!(kind, TextKind::Code | TextKind::Address) && text.is_empty() {
        let paste = Rect::from_min_max(pos2(inner.right() - 56.0 * scale, rect.top()), pos2(rect.right(), rect.bottom()));
        let response = ui.interact(paste, ui.id().with((label, "paste")), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), "Paste"));
        let color = if response.hovered() || response.has_focus() { Color32::WHITE } else { MUTED };
        ui.painter().text(paste.center(), Align2::CENTER_CENTER, "Paste", FontId::proportional(13.0 * scale), color);
        if response.clicked()
            && let Some(pasted) = super::clipboard_text()
        {
            *text = pasted.trim().chars().take(max).collect();
        }
        inner.max.x = paste.left();
    }
    let edit_rect = Rect::from_min_max(pos2(inner.left() + label_width, inner.top()), inner.max);
    let edit = ui
        .scope_builder(UiBuilder::new().max_rect(edit_rect).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.add(
                TextEdit::singleline(text)
                    .frame(Frame::NONE)
                    .hint_text(RichText::new(hint).color(Color32::from_gray(100)))
                    .char_limit(max)
                    .font(FontId::proportional(15.0 * scale))
                    .margin(Vec2::ZERO)
                    .desired_width(edit_rect.width()),
            )
        })
        .inner;
    if field.clicked() {
        edit.request_focus();
    }
    if edit.has_focus() {
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.5, Color32::WHITE), StrokeKind::Inside);
        ui.ctx().data_mut(|d| d.insert_temp(Id::new(super::TEXT_KIND), kind));
    }
    focus(edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;

    #[test]
    fn slider_keyboard_keeps_focus_and_reports_changes() {
        let ctx = Context::default();
        theme::install(&ctx);
        let mut value = 0.75;
        let mut id = Id::NULL;
        let mut changed = false;
        for frame in 0..3 {
            let events = if frame == 2 {
                vec![Event::Key {
                    key: Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }]
            } else {
                vec![]
            };
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(390.0, 844.0))),
                    events,
                    ..Default::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        let response = slider(ui, &mut value, 0.5..=1.0, 0.05, "%", 0);
                        id = response.id;
                        if frame < 2 {
                            response.request_focus();
                        }
                        changed = response.changed();
                    });
                },
            );
            output.textures_delta.clear();
        }
        assert!((value - 0.8).abs() < 0.001);
        assert!(changed);
        assert_eq!(ctx.memory(|m| m.focused()), Some(id));
    }

    #[test]
    fn controls_and_action_buttons_share_the_right_edge() {
        for width in [248.0, 318.0, 640.0] {
            let ctx = Context::default();
            theme::install(&ctx);
            let mut rect = Rect::NOTHING;
            let mut right = 0.0;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width + 40.0, 500.0))),
                    ..Default::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        ui.set_width(width);
                        right = ui.max_rect().right() - 12.0;
                        setting(
                            ui,
                            "Game folder",
                            Control::Action,
                            Some("A very long folder path to truncate"),
                            |ui| {
                                rect = action_button(ui, "Choose…").rect;
                            },
                        );
                        assert!((rect.right() - right).abs() < 1.0, "{width}: {rect:?}, {:?}", ui.min_rect());
                    });
                },
            );
            output.textures_delta.clear();
        }
    }
}
