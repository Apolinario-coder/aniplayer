use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle, Vec2, Visuals};

pub const PAGE: Color32 = Color32::from_rgb(18, 15, 31);
pub const STAGE: Color32 = Color32::from_rgb(26, 21, 44);
pub const PANEL: Color32 = Color32::from_rgb(34, 27, 54);
pub const CONTROL: Color32 = Color32::from_rgb(49, 38, 73);
pub const HOVER: Color32 = Color32::from_rgb(65, 49, 91);
pub const BORDER: Color32 = Color32::from_rgb(79, 62, 108);
pub const INPUT_EDGE: Color32 = Color32::from_rgb(157, 134, 187);
pub const INK: Color32 = Color32::from_rgb(253, 242, 255);
pub const MUTED: Color32 = Color32::from_rgb(208, 191, 226);
pub const SAKURA: Color32 = Color32::from_rgb(255, 176, 210);
pub const ON_SAKURA: Color32 = Color32::from_rgb(45, 21, 49);
pub const ERROR: Color32 = Color32::from_rgb(255, 173, 184);
pub const GAP: f32 = 8.0;
pub const INSET: i8 = 20;
pub const CORNER: u8 = 12;

pub fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(GAP, GAP);
    style.spacing.button_padding = Vec2::new(11.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.animation_time = 0.0;
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(25.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(INK);
    visuals.panel_fill = PAGE;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = PAGE;
    visuals.faint_bg_color = STAGE;
    visuals.selection.bg_fill = HOVER;
    visuals.selection.stroke = Stroke::new(1.0_f32, SAKURA);
    visuals.error_fg_color = ERROR;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
        &mut visuals.widgets.noninteractive,
    ] {
        widget.bg_fill = CONTROL;
        widget.weak_bg_fill = CONTROL;
        widget.bg_stroke = Stroke::new(1.0_f32, INPUT_EDGE);
        widget.fg_stroke = Stroke::new(1.0_f32, INK);
        widget.corner_radius = CORNER.into();
    }
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, SAKURA);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, SAKURA);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals = visuals;
    ctx.set_style(style);
}

pub fn panel(fill: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .inner_margin(egui::Margin::same(INSET))
}

pub fn primary(label: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).strong().color(ON_SAKURA))
        .fill(SAKURA)
        .corner_radius(CORNER)
}
