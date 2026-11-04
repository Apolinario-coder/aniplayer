use super::{Player, Tab, theme::*};
use crate::i18n::tr;
use eframe::egui::{self, Align, Layout, RichText};
use std::time::{Duration, Instant};

const EDGE_WIDTH: f32 = 12.0;
const PANEL_WIDTH: f32 = 370.0;
const SLIDE_DURATION: f32 = 0.28;
const HIDE_DELAY: Duration = Duration::from_millis(1200);

pub(super) struct Dock {
    pub open: bool,
    tab: Tab,
    last_inside: Instant,
    armed: bool,
    progress: f32,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Motion,
            last_inside: Instant::now(),
            armed: true,
            progress: 0.0,
        }
    }
}

impl Dock {
    pub fn close(&mut self) {
        self.open = false;
        self.armed = false;
    }

    fn track(&mut self, ctx: &egui::Context, width: f32) {
        let screen = ctx.screen_rect();
        let position = ctx.input(|i| i.pointer.hover_pos());
        let edge = position
            .is_some_and(|pos| pos.x >= screen.right() - EDGE_WIDTH && screen.contains(pos));
        if !edge {
            self.armed = true;
        }
        if edge && self.armed && ctx.input(|i| i.focused) {
            self.open = true;
        }
        if !self.open {
            return;
        }
        let inside =
            position.is_some_and(|pos| pos.x >= screen.right() - width && screen.contains(pos));
        let interacting = ctx.memory(|m| m.any_popup_open()) || ctx.wants_keyboard_input();
        if inside || interacting {
            self.last_inside = Instant::now();
        } else if self.last_inside.elapsed() >= HIDE_DELAY {
            self.close();
        }
    }
}

impl Player {
    pub(super) fn fullscreen_settings(&mut self, ctx: &egui::Context) {
        if !self.fullscreen_video() || self.hub.is_open() {
            self.dock.close();
            self.dock.progress = 0.0;
            return;
        }
        let width = PANEL_WIDTH.min(ctx.screen_rect().width() * 0.48);
        self.dock.track(ctx, width);
        let step = ctx.input(|i| i.stable_dt).min(0.05) / SLIDE_DURATION;
        self.dock.progress = if self.dock.open {
            (self.dock.progress + step).min(1.0)
        } else {
            (self.dock.progress - step).max(0.0)
        };
        if self.dock.progress <= 0.0 {
            return;
        }
        let progress = self.dock.progress;
        let eased = progress * progress * (3.0 - 2.0 * progress);
        let visible_width = width * eased;
        if progress < 1.0 {
            ctx.request_repaint();
        }
        egui::SidePanel::right("fullscreen_settings")
            .min_width(0.0)
            .exact_width(visible_width)
            .resizable(false)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let available = ui.max_rect();
                let left = if self.dock.open {
                    available.right() - visible_width
                } else {
                    available.right() - width - width * (1.0 - eased)
                };
                let origin = egui::pos2(left, available.top());
                let drawer =
                    egui::Rect::from_min_size(origin, egui::vec2(width, available.height()));
                let clip = available.intersect(ui.clip_rect());
                let mut content =
                    ui.new_child(egui::UiBuilder::new().max_rect(drawer.shrink(f32::from(INSET))));
                content.set_clip_rect(clip);
                content.multiply_opacity(eased);
                content.painter().rect_filled(drawer, 0, PANEL);
                if !self.dock.open {
                    content.disable();
                }
                let ui = &mut content;
                ui.horizontal(|ui| {
                    ui.heading(tr("Configurações"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr("Fechar")).clicked() {
                            self.dock.close();
                        }
                    });
                });
                ui.label(
                    RichText::new(tr("Ajuste enquanto assiste ao episódio."))
                        .small()
                        .color(MUTED),
                );
                ui.add_space(GAP);
                ui.horizontal_wrapped(|ui| {
                    for (tab, label) in [
                        (Tab::Motion, tr("Fluidez")),
                        (Tab::Tracks, tr("Áudio e legendas")),
                        (Tab::Tools, tr("Imagem")),
                    ] {
                        if ui.selectable_label(self.dock.tab == tab, label).clicked() {
                            self.dock.tab = tab;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(("fullscreen_settings", self.dock.tab as u8))
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.dock.tab {
                        Tab::Tracks => self.tracks(ui),
                        Tab::Tools => self.tools(ui),
                        _ => self.motion_panel(ui),
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_reveals_panel_and_leaving_hides_it() {
        let ctx = egui::Context::default();
        let mut dock = Dock::default();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 720.0),
            )),
            focused: true,
            ..Default::default()
        };
        input
            .events
            .push(egui::Event::PointerMoved(egui::pos2(1278.0, 300.0)));
        let _ = ctx.run(input, |ctx| dock.track(ctx, PANEL_WIDTH));
        assert!(dock.open);
        dock.last_inside = Instant::now() - HIDE_DELAY - Duration::from_millis(1);
        let mut input = egui::RawInput::default();
        input
            .events
            .push(egui::Event::PointerMoved(egui::pos2(500.0, 300.0)));
        let _ = ctx.run(input, |ctx| dock.track(ctx, PANEL_WIDTH));
        assert!(!dock.open);
    }
}
