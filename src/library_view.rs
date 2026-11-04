use super::{Player, library::Episode, theme::*};
use crate::i18n::tr;
use eframe::egui::{self, Align, FontId, Layout, Rect, RichText, Stroke, Vec2};
use serde_json::json;
use std::path::Path;

impl Player {
    pub(super) fn library_ui(&mut self, ui: &mut egui::Ui, large: bool) {
        let mut controls = Vec::new();
        if let Some((message, true)) = &self.notice {
            ui.colored_label(ERROR, message);
        }
        if let Some(error) = &self.library.error {
            ui.colored_label(ERROR, error);
        }
        ui.horizontal_wrapped(|ui| {
            if self.library.folder != 0 && ui.button(tr("Pasta anterior")).clicked() {
                self.library.folder = self.library.parent();
            }
            let title = ui.label(RichText::new(self.library.title()).strong().color(SAKURA));
            if self.library.folder != 0 {
                title.context_menu(|ui| {
                    if ui
                        .add_enabled(
                            self.library.error.is_none(),
                            egui::Button::new(tr("Excluir esta pasta")),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.confirm_delete_folder(self.library.folder);
                    }
                });
            }
        });
        ui.horizontal_wrapped(|ui| {
            let available = self.ready && self.library.error.is_none();
            let add = ui.add_enabled(available, egui::Button::new(tr("Adicionar episódios")));
            if available {
                controls.push((add.id, add.rect));
            }
            if add.clicked() || (self.hub.confirm_selection() && add.has_focus()) {
                if self.fullscreen && self.hub.is_open() {
                    self.cmd(&["set", "pause", "yes"]);
                }
                self.pick(true);
            }
            if self
                .value("playlist")
                .as_array()
                .is_some_and(|items| items.len() > 1)
                && ui.button(tr("Embaralhar fila")).clicked()
            {
                self.cmd(&["playlist-shuffle"]);
            }
            if ui
                .add_enabled(self.ready, egui::Button::new(tr("Abrir playlist M3U")))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(tr("Playlist M3U"), &["m3u", "m3u8"])
                    .pick_file()
                {
                    self.send(vec![
                        json!("loadlist"),
                        json!(path.to_string_lossy()),
                        json!("replace"),
                    ]);
                }
            }
        });
        ui.add_enabled_ui(self.library.error.is_none(), |ui| {
            ui.horizontal(|ui| {
                let label = ui.label(tr("Nova pasta"));
                let field = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.library.folder_name)
                            .hint_text(tr("Anime ou temporada"))
                            .desired_width(if large {
                                240.0
                            } else {
                                (ui.available_width() - 64.0).max(40.0)
                            }),
                    )
                    .labelled_by(label.id);
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui
                    .add_enabled(
                        !self.library.folder_name.trim().is_empty(),
                        egui::Button::new(tr("Criar")),
                    )
                    .clicked()
                    || enter
                {
                    self.library.create_folder();
                }
            });
        });
        ui.add_space(GAP);
        let folders: Vec<_> = self
            .library
            .data
            .folders
            .iter()
            .filter(|folder| folder.parent == self.library.folder)
            .cloned()
            .collect();
        for folder in folders {
            let count = self
                .library
                .data
                .episodes
                .iter()
                .filter(|e| e.folder == folder.id)
                .count();
            let (button, remove) = ui
                .push_id(("folder", folder.id), |ui| {
                    folder_row(ui, &folder.name, count, large, self.library.error.is_none())
                })
                .inner;
            if remove {
                self.confirm_delete_folder(folder.id);
            }
            controls.push((button.id, button.rect));
            if button.clicked() || (self.hub.confirm_selection() && button.has_focus()) {
                self.library.folder = folder.id;
                self.hub.refresh_library_focus();
            }
        }
        let episodes: Vec<_> = self
            .library
            .data
            .episodes
            .iter()
            .filter(|e| e.folder == self.library.folder)
            .cloned()
            .collect();
        if episodes.is_empty() {
            ui.add_space(GAP);
            ui.label(tr("Esta pasta ainda não tem episódios."));
            ui.label(
                RichText::new(tr("Adicione arquivos ou mova episódios de outra pasta."))
                    .small()
                    .color(MUTED),
            );
        }
        let columns = if large && ui.available_width() >= 1000.0 {
            3
        } else if large && ui.available_width() >= 660.0 {
            2
        } else {
            1
        };
        let width = (ui.available_width() - GAP * (columns - 1) as f32) / columns as f32;
        egui::ScrollArea::vertical().id_salt(("library", self.library.folder, large)).auto_shrink([false, false]).show(ui, |ui| {
            for row in episodes.chunks(columns) {
                ui.horizontal(|ui| {
                    for episode in row {
                        ui.push_id(episode.id, |ui| {
                            let current = self.text("path") == episode.path;
                            let available = Path::new(&episode.path).is_file() || episode.path.starts_with("http://") || episode.path.starts_with("https://");
                            let response = episode_card(ui, episode, width, large, current, available);
                            controls.push((response.id, response.rect));
                            let image_width = if large { 156.0 } else { 88.0 };
                            let image_rect = Rect::from_min_size(response.rect.left_center() + Vec2::new(10.0, -image_width * 9.0 / 32.0), Vec2::new(image_width, image_width * 9.0 / 16.0));
                            if ui.is_rect_visible(response.rect) {
                                self.previews.paint(ui, &episode.path, image_rect, response.hovered());
                            }
                            if self.ready && (response.clicked() || (self.hub.confirm_selection() && response.has_focus())) {
                                if available {
                                    self.play_library_episode(episode.id);
                                    if self.fullscreen { self.resume_fullscreen(); }
                                } else {
                                    self.notice = Some((tr("O arquivo não foi encontrado. Verifique se a unidade está conectada.").into(), true));
                                }
                            }
                            response.context_menu(|ui| {
                                ui.menu_button(tr("Mover para pasta"), |ui| {
                                    let mut destinations = vec![(0, tr("Todos os animes").to_owned())];
                                    destinations.extend(self.library.data.folders.iter().map(|f| (f.id, self.library.folder_title(f.id))));
                                    for (id, name) in destinations {
                                        if ui.add_enabled(episode.folder != id, egui::Button::new(name)).clicked() {
                                            self.library.move_episode(episode.id, id);
                                            ui.close_menu();
                                        }
                                    }
                                });
                                if ui.button(tr("Remover da biblioteca")).clicked() {
                                    self.library.remove_episode(episode.id);
                                    ui.close_menu();
                                }
                            });
                        });
                    }
                });
            }
        });
        ui.label(RichText::new(tr("Passe o mouse para ver momentos do episódio. Botão direito para mover entre pastas.")).small().color(MUTED));
        if large {
            self.hub.library_controls(ui.ctx(), controls);
        }
        self.hub.clear_selection();
    }

    fn confirm_delete_folder(&mut self, id: u64) {
        if self.library.error.is_none() {
            self.delete_folder = Some(id);
        }
    }

    pub(super) fn folder_delete_modal(&mut self, ctx: &egui::Context) {
        let Some(id) = self.delete_folder else { return };
        let Some(folder) = self
            .library
            .data
            .folders
            .iter()
            .find(|folder| folder.id == id)
        else {
            self.delete_folder = None;
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("delete_library_folder"))
            .backdrop_color(PAGE.gamma_multiply(0.80))
            .frame(panel(PANEL).inner_margin(egui::Margin::same(24))
                .corner_radius(CORNER).stroke(Stroke::new(1.0_f32, BORDER)))
            .show(ctx, |ui| {
                ui.set_width(420.0_f32.min(ctx.screen_rect().width() - 80.0));
                ui.label(RichText::new(tr("BIBLIOTECA")).small().strong().color(SAKURA));
                ui.heading(tr("Excluir pasta?"));
                ui.add_space(GAP);
                ui.label(RichText::new(&folder.name).strong());
                ui.label(RichText::new(tr("A pasta e suas subpastas sairão da biblioteca. Os episódios serão movidos para a pasta anterior.")).color(MUTED));
                ui.add_space(GAP * 2.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let delete = ui.add(egui::Button::new(RichText::new(tr("Excluir pasta")).strong().color(ON_SAKURA))
                        .fill(ERROR).min_size(Vec2::new(110.0, 38.0)));
                    let cancel = ui.add(egui::Button::new(tr("Cancelar")).min_size(Vec2::new(90.0, 38.0)));
                    if !ui.memory(|memory| memory.has_focus(delete.id) || memory.has_focus(cancel.id)) {
                        cancel.request_focus();
                    }
                    (delete.clicked(), cancel.clicked())
                }).inner
            });
        let (delete, cancel) = modal.inner;
        if delete {
            self.library.remove_folder(id);
        }
        if delete || cancel || modal.should_close() {
            self.delete_folder = None;
            self.hub.refresh_library_focus();
        }
    }

    pub(super) fn play_library_episode(&mut self, id: u64) {
        let episodes: Vec<_> = self
            .library
            .data
            .episodes
            .iter()
            .filter(|episode| episode.folder == self.library.folder)
            .cloned()
            .collect();
        let Some(index) = episodes.iter().position(|episode| episode.id == id) else {
            return;
        };
        // Start the selected episode, followed by the remaining episodes in this folder.
        for (offset, episode) in episodes.iter().skip(index).enumerate() {
            self.send(vec![
                json!("loadfile"),
                json!(&episode.path),
                json!(if offset == 0 { "replace" } else { "append" }),
            ]);
        }
        self.cmd(&["set", "pause", "no"]);
    }
}

fn episode_card(
    ui: &mut egui::Ui,
    episode: &Episode,
    width: f32,
    large: bool,
    current: bool,
    available: bool,
) -> egui::Response {
    let thumbnail_width = if large { 156.0 } else { 88.0 };
    let height = if large { 112.0 } else { 76.0 };
    let button = egui::Button::new("")
        .selected(current)
        .min_size(Vec2::new(width, height))
        .corner_radius(CORNER);
    let response = ui.add(button);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!(
                "{}{}",
                episode.title,
                if current {
                    format!(". {}", tr("Em reprodução"))
                } else {
                    String::new()
                }
            ),
        )
    });
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            CORNER,
            Stroke::new(2.0_f32, SAKURA),
            egui::StrokeKind::Inside,
        );
    }
    let text_rect = Rect::from_min_max(
        response.rect.min + Vec2::new(thumbnail_width + 24.0, 12.0),
        response.rect.max - Vec2::new(10.0, 10.0),
    );
    let mut content = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    content.set_clip_rect(text_rect.intersect(ui.clip_rect()));
    content.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(&episode.title)
                    .font(FontId::proportional(if large { 20.0 } else { 14.0 }))
                    .strong(),
            )
            .truncate(),
        );
        ui.label(
            RichText::new(if !available {
                tr("Arquivo indisponível")
            } else if current {
                tr("Em reprodução")
            } else {
                tr("Reproduzir episódio")
            })
            .small()
            .color(MUTED),
        );
    });
    response.on_hover_text(&episode.title)
}

fn folder_row(
    ui: &mut egui::Ui,
    name: &str,
    count: usize,
    large: bool,
    editable: bool,
) -> (egui::Response, bool) {
    let mut remove = false;
    let height = if large { 58.0 } else { 40.0 };
    let delete_width = if large { 90.0 } else { 64.0 };
    let response = ui
        .horizontal(|ui| {
            let width =
                (ui.available_width() - delete_width - ui.spacing().item_spacing.x).max(40.0);
            let icon = egui::Image::new(egui::include_image!("../assets/icons/folder-open.svg"))
                .fit_to_exact_size(Vec2::splat(if large { 30.0 } else { 22.0 }))
                .tint(SAKURA);
            let button = ui.add_sized(
                [width, height],
                egui::Button::image_and_text(icon, format!("{name} · {count} {}", tr("episódios")))
                    .truncate(),
            );
            button.context_menu(|ui| {
                if ui
                    .add_enabled(editable, egui::Button::new(tr("Excluir pasta")))
                    .clicked()
                {
                    remove = true;
                    ui.close_menu();
                }
            });
            if ui
                .add_enabled(
                    editable,
                    egui::Button::new(tr("Excluir")).min_size(Vec2::new(delete_width, height)),
                )
                .on_hover_text(tr("Excluir pasta da biblioteca"))
                .clicked()
            {
                remove = true;
            }
            button
        })
        .inner;
    (response, remove)
}

#[cfg(test)]
mod folder_tests {
    use super::*;

    #[test]
    fn right_click_opens_subfolder_menu() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        egui_extras::install_image_loaders(&ctx);
        let mut rect = Rect::NOTHING;
        let mut opened = false;
        let render = |ctx: &egui::Context, rect: &mut Rect, opened: &mut bool| {
            egui::CentralPanel::default().show(ctx, |ui| {
                *rect = ui
                    .push_id(("folder", 27_u64), |ui| {
                        let button = folder_row(ui, "Temporada 2", 12, false, true).0;
                        *opened = button.context_menu_opened();
                        button.rect
                    })
                    .inner;
            });
        };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            render(ctx, &mut rect, &mut opened)
        });
        let position = rect.center();
        for pressed in [true, false] {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::PointerMoved(position));
            input.events.push(egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            let _ = ctx.run(input, |ctx| render(ctx, &mut rect, &mut opened));
        }
        assert!(opened);
    }
}
