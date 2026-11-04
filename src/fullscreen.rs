use super::{Player, theme::*, track_label};
use crate::i18n::tr;
use eframe::egui::{
    self, Align, FontId, Key, Layout, Modifiers, Rect, RichText, Sense, Stroke, Vec2,
};
#[cfg(unix)]
use no_gamepad::*;
#[cfg(windows)]
use windows_sys::Win32::UI::Input::XboxController::*;

#[cfg(unix)]
mod no_gamepad {
    pub const XINPUT_GAMEPAD_DPAD_UP: u16 = 1;
    pub const XINPUT_GAMEPAD_DPAD_DOWN: u16 = 2;
    pub const XINPUT_GAMEPAD_DPAD_LEFT: u16 = 4;
    pub const XINPUT_GAMEPAD_DPAD_RIGHT: u16 = 8;
    pub const XINPUT_GAMEPAD_START: u16 = 16;
    pub const XINPUT_GAMEPAD_LEFT_SHOULDER: u16 = 256;
    pub const XINPUT_GAMEPAD_RIGHT_SHOULDER: u16 = 512;
    pub const XINPUT_GAMEPAD_A: u16 = 4096;
    pub const XINPUT_GAMEPAD_B: u16 = 8192;
    pub const XINPUT_GAMEPAD_X: u16 = 16384;
}

const TILE_HEIGHT: f32 = 82.0;
const TILE_GAP: f32 = 16.0;
const HERO_HEIGHT: f32 = 640.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Home,
    Episodes,
    Tracks,
    Motion,
    Upscale,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Home => tr("Sua sessão de anime"),
            Self::Episodes => tr("Episódios na fila"),
            Self::Tracks => tr("Áudio e legendas"),
            Self::Motion => tr("Fluidez"),
            Self::Upscale => "Anime4K",
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    Resume,
    Open { append: bool },
    Page(Page),

    Track(&'static str, Option<i64>),
    Motion(&'static str),
    Upscale(crate::upscale::Mode),
    ExitFullscreen,
    Quit,
}

struct Tile {
    title: String,
    detail: String,
    action: Action,
    enabled: bool,
    selected: bool,
}

impl Tile {
    fn new(title: impl Into<String>, detail: impl Into<String>, action: Action) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            action,
            enabled: true,
            selected: false,
        }
    }

    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

#[derive(Default)]
pub(super) struct Hub {
    open: bool,
    page: Page,
    cursor: usize,
    home_cursor: usize,
    library_focus: Vec<(egui::Id, Rect)>,
    direction: Option<Direction>,
    confirm: bool,
    refocus: bool,
    pub(super) resume_on_exit: bool,
    #[cfg(windows)]
    pad: Option<(u32, u16)>,
}

#[derive(Clone, Copy)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Hub {
    pub(super) fn refresh_library_focus(&mut self) {
        self.refocus = true;
    }

    pub(super) fn library_controls(
        &mut self,
        ctx: &egui::Context,
        controls: Vec<(egui::Id, Rect)>,
    ) {
        if self.refocus {
            if let Some((id, _)) = controls.first() {
                ctx.memory_mut(|memory| memory.request_focus(*id));
            }
            self.refocus = false;
        }
        self.library_focus = controls;
    }

    fn library_direction(&self, ctx: &egui::Context, direction: Direction) {
        let focused = ctx.memory(|memory| memory.focused());
        let current = self
            .library_focus
            .iter()
            .find(|(id, _)| Some(*id) == focused);
        let Some((_, origin)) = current else {
            if let Some((id, _)) = self.library_focus.first() {
                ctx.memory_mut(|memory| memory.request_focus(*id));
            }
            return;
        };
        let next = self
            .library_focus
            .iter()
            .filter(|(id, rect)| {
                if Some(*id) == focused {
                    return false;
                }
                let delta = rect.center() - origin.center();
                match direction {
                    Direction::Left => delta.x < -1.0,
                    Direction::Right => delta.x > 1.0,
                    Direction::Up => delta.y < -1.0,
                    Direction::Down => delta.y > 1.0,
                }
            })
            .min_by(|(_, a), (_, b)| {
                let score = |rect: Rect| {
                    let delta = rect.center() - origin.center();
                    match direction {
                        Direction::Left | Direction::Right => delta.x.abs() + delta.y.abs() * 3.0,
                        Direction::Up | Direction::Down => delta.y.abs() + delta.x.abs() * 3.0,
                    }
                };
                score(*a).total_cmp(&score(*b))
            });
        if let Some((id, _)) = next {
            ctx.memory_mut(|memory| memory.request_focus(*id));
        }
    }

    pub(super) fn confirm_selection(&self) -> bool {
        self.confirm
    }

    pub(super) fn clear_selection(&mut self) {
        self.confirm = false;
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn close(&mut self) {
        self.open = false;
        self.resume_on_exit = false;
        self.direction = None;
        self.confirm = false;
    }

    fn go(&mut self, page: Page) {
        if self.page == Page::Home && page != Page::Home {
            self.home_cursor = self.cursor;
        }
        self.page = page;
        self.cursor = if page == Page::Home {
            self.home_cursor
        } else {
            0
        };
        self.refocus = true;
        self.direction = None;
        self.confirm = false;
    }

    #[cfg(windows)]
    fn pad_buttons(&mut self, focused: bool) -> u16 {
        let mut state = std::mem::MaybeUninit::<XINPUT_STATE>::uninit();
        let current = (0..4).find_map(|index| {
            if unsafe { XInputGetState(index, state.as_mut_ptr()) } != 0 {
                return None;
            }
            let pad = unsafe { state.assume_init() }.Gamepad;
            let mut mask = pad.wButtons;
            if pad.sThumbLX < -16000 {
                mask |= XINPUT_GAMEPAD_DPAD_LEFT;
            }
            if pad.sThumbLX > 16000 {
                mask |= XINPUT_GAMEPAD_DPAD_RIGHT;
            }
            if pad.sThumbLY < -16000 {
                mask |= XINPUT_GAMEPAD_DPAD_DOWN;
            }
            if pad.sThumbLY > 16000 {
                mask |= XINPUT_GAMEPAD_DPAD_UP;
            }
            Some((index, mask))
        });
        let previous = std::mem::replace(&mut self.pad, current);
        match (focused, previous, current) {
            (true, Some((before, old)), Some((now, new))) if before == now => new & !old,
            _ => 0,
        }
    }
}

impl Player {
    pub(super) fn show_hub(&mut self) {
        self.dock.close();
        self.hub.open = true;
        self.hub.home_cursor = 0;
        self.hub.go(
            if self.preview.is_some() && std::env::var_os("ANI_PREVIEW_LIBRARY").is_some() {
                Page::Episodes
            } else {
                Page::Home
            },
        );
        #[cfg(windows)]
        if let Some(surface) = &mut self.surface {
            surface.hide();
        }
        self.hub.resume_on_exit = self.loaded() && !self.yes("pause");
        #[cfg(unix)]
        if self.loaded() {
            self.cmd(&["set", "fullscreen", "no"]);
        }
        if self.hub.resume_on_exit {
            self.cmd(&["set", "pause", "yes"]);
        }
    }

    pub(super) fn resume_fullscreen(&mut self) {
        self.hub.close();
        #[cfg(unix)]
        self.cmd(&["set", "fullscreen", "yes"]);
        self.cmd(&["set", "pause", "no"]);
    }

    pub(super) fn fullscreen_input(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        let pad = self.hub.pad_buttons(ctx.input(|i| i.focused));
        #[cfg(unix)]
        let pad = 0;
        let key = |key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        if key(Key::Escape) || pad & (XINPUT_GAMEPAD_B | XINPUT_GAMEPAD_START) != 0 {
            if self.dock.open {
                self.dock.close();
            } else if !self.hub.open {
                self.show_hub();
            } else if self.hub.page != Page::Home {
                self.hub.go(Page::Home);
            } else {
                self.fullscreen(ctx);
            }
            return;
        }
        if self.dock.open {
            return;
        }
        if self.hub.open && self.hub.page == Page::Episodes {
            for (button, direction) in [
                (XINPUT_GAMEPAD_DPAD_LEFT, Direction::Left),
                (XINPUT_GAMEPAD_DPAD_RIGHT, Direction::Right),
                (XINPUT_GAMEPAD_DPAD_UP, Direction::Up),
                (XINPUT_GAMEPAD_DPAD_DOWN, Direction::Down),
            ] {
                if pad & button != 0 {
                    self.hub.library_direction(ctx, direction);
                }
            }
            self.hub.confirm = pad & XINPUT_GAMEPAD_A != 0;
            return;
        }
        let directions = [
            (Key::ArrowLeft, XINPUT_GAMEPAD_DPAD_LEFT, Direction::Left),
            (Key::ArrowRight, XINPUT_GAMEPAD_DPAD_RIGHT, Direction::Right),
            (Key::ArrowUp, XINPUT_GAMEPAD_DPAD_UP, Direction::Up),
            (Key::ArrowDown, XINPUT_GAMEPAD_DPAD_DOWN, Direction::Down),
        ];
        if self.hub.open {
            self.hub.direction = directions
                .into_iter()
                .find_map(|(k, b, d)| (key(k) || pad & b != 0).then_some(d));
            self.hub.confirm = pad & XINPUT_GAMEPAD_A != 0;
        } else {
            if key(Key::Space) || pad & XINPUT_GAMEPAD_A != 0 {
                self.cmd(&["cycle", "pause"]);
            }
            if key(Key::M) || pad & XINPUT_GAMEPAD_X != 0 {
                self.cmd(&["cycle", "mute"]);
            }
            if key(Key::ArrowLeft)
                || pad & (XINPUT_GAMEPAD_DPAD_LEFT | XINPUT_GAMEPAD_LEFT_SHOULDER) != 0
            {
                self.cmd(&["seek", "-5", "relative"]);
            }
            if key(Key::ArrowRight)
                || pad & (XINPUT_GAMEPAD_DPAD_RIGHT | XINPUT_GAMEPAD_RIGHT_SHOULDER) != 0
            {
                self.cmd(&["seek", "5", "relative"]);
            }
        }
    }

    fn hub_tiles(&self) -> Vec<Tile> {
        match self.hub.page {
            Page::Home => {
                let first = if self.loaded() {
                    Tile::new(
                        tr("Continuar assistindo"),
                        tr("Voltar ao vídeo em tela cheia"),
                        Action::Resume,
                    )
                } else {
                    Tile::new(
                        tr("Abrir episódio"),
                        tr("Escolher um arquivo de vídeo"),
                        Action::Open { append: false },
                    )
                    .enabled(self.ready)
                };
                let mut tiles = vec![
                    first,
                    Tile::new(
                        tr("Episódios"),
                        tr("Escolher na fila de reprodução"),
                        Action::Page(Page::Episodes),
                    ),
                    Tile::new(
                        tr("Áudio e legendas"),
                        tr("Trocar as faixas do episódio"),
                        Action::Page(Page::Tracks),
                    )
                    .enabled(self.loaded()),
                    Tile::new(
                        tr("Fluidez"),
                        tr("Selecionar a taxa de quadros"),
                        Action::Page(Page::Motion),
                    )
                    .enabled(self.ready),
                    Tile::new(
                        "Anime4K",
                        tr("Ajustar o upscaling na GPU"),
                        Action::Page(Page::Upscale),
                    )
                    .enabled(self.ready),
                ];
                if self.loaded() {
                    tiles.push(Tile::new(
                        tr("Abrir outro episódio"),
                        tr("Escolher um arquivo de vídeo"),
                        Action::Open { append: false },
                    ));
                }
                tiles.push(Tile::new(
                    tr("Sair da tela cheia"),
                    tr("Voltar à janela do player"),
                    Action::ExitFullscreen,
                ));
                tiles.push(Tile::new(
                    tr("Sair do Ani Player"),
                    tr("Fechar o programa"),
                    Action::Quit,
                ));
                tiles
            }
            Page::Episodes => Vec::new(),
            Page::Tracks => {
                let mut tiles = Vec::new();
                for (kind, property, label) in [
                    ("audio", "aid", tr("Áudio")),
                    ("sub", "sid", tr("Legendas")),
                ] {
                    let tracks = self
                        .value("track-list")
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|track| track["type"] == kind);
                    let mut enabled = false;
                    for track in tracks {
                        let Some(id) = track["id"].as_i64() else {
                            continue;
                        };
                        let selected = track["selected"] == true;
                        enabled |= selected;
                        tiles.push(
                            Tile::new(track_label(track), label, Action::Track(property, Some(id)))
                                .selected(selected),
                        );
                    }
                    tiles.push(
                        Tile::new(
                            format!("{label} · {}", tr("Desligado")),
                            tr("Desativar faixa"),
                            Action::Track(property, None),
                        )
                        .selected(!enabled),
                    );
                }
                tiles
            }
            Page::Motion => crate::motion::MODES
                .iter()
                .map(|mode| {
                    Tile::new(
                        tr(mode),
                        if *mode == "Original" {
                            tr("Taxa de origem")
                        } else {
                            tr("Interpolação na GPU")
                        },
                        Action::Motion(mode),
                    )
                    .selected(self.mode == *mode)
                })
                .collect(),
            Page::Upscale => crate::upscale::Mode::ALL
                .into_iter()
                .map(|mode| {
                    Tile::new(mode.label(), "Anime4K · GPU", Action::Upscale(mode)).selected(
                        crate::upscale::Mode::current(
                            &self.paths.config,
                            self.value("glsl-shaders"),
                        ) == mode,
                    )
                })
                .collect(),
        }
    }

    fn hub_action(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Resume => self.resume_fullscreen(),
            Action::Open { append } => {
                if self.pick(append) && !append {
                    self.resume_fullscreen();
                }
            }
            Action::Page(page) => self.hub.go(page),
            Action::Track(property, id) => {
                let id = id.map_or_else(|| "no".into(), |id| id.to_string());
                self.cmd(&["set", property, &id]);
            }
            Action::Motion(mode) => self.select_mode(mode),
            Action::Upscale(mode) => {
                if let Some(engine) = &self.engine {
                    engine.upscale(mode);
                }
            }
            Action::ExitFullscreen => self.fullscreen(ctx),
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    pub(super) fn fullscreen_hub(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        if let Some(surface) = &mut self.surface {
            surface.hide();
        }
        let page = self.hub.page;
        let screen = ctx.screen_rect();
        let hero_height = (screen.height() * 0.43).clamp(180.0, HERO_HEIGHT);
        let scene = self.scene.as_ref().filter(|_| page == Page::Home);
        let background = ctx.layer_painter(egui::LayerId::background());
        // Reserve the artwork below the header and content, then size it from the panel layout.
        let artwork = scene.map(|_| {
            background.rect_filled(screen, 0, PAGE);
            background.add(egui::Shape::Noop)
        });
        let mut tiles = self.hub_tiles();
        if page != Page::Home {
            tiles.push(Tile::new(
                tr("Voltar ao início"),
                tr("Menu de tela cheia"),
                Action::Page(Page::Home),
            ));
        }
        egui::TopBottomPanel::top("hub_header")
            .frame(if artwork.is_some() {
                panel(egui::Color32::TRANSPARENT).stroke(Stroke::NONE)
            } else {
                panel(PANEL)
            })
            .show_separator_line(artwork.is_none())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Ani Player");
                    ui.label(
                        RichText::new(tr("TELA CHEIA"))
                            .small()
                            .strong()
                            .color(SAKURA),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        crate::i18n::menu(ui, &self.paths.config);
                        if ui
                            .button(if page == Page::Home {
                                tr("Sair da tela cheia")
                            } else {
                                tr("Voltar")
                            })
                            .clicked()
                        {
                            if page == Page::Home {
                                self.fullscreen(ctx);
                            } else {
                                self.hub.go(Page::Home);
                            }
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("hub_footer")
            .frame(panel(PANEL))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(tr("Setas / direcional  Navegar")).strong());
                    ui.label(RichText::new(tr("Enter / A  Selecionar")).color(MUTED));
                    ui.label(
                        RichText::new(if page == Page::Home {
                            tr("Esc / B  Sair da tela cheia")
                        } else {
                            tr("Esc / B  Voltar")
                        })
                        .color(MUTED),
                    );
                });
            });
        let mut action = None;
        let large = ctx.screen_rect().width() >= 1500.0;
        let frame = if artwork.is_some() {
            panel(egui::Color32::TRANSPARENT).stroke(Stroke::NONE)
        } else {
            panel(PAGE)
        }
        .inner_margin(egui::Margin::same(if large { 44 } else { INSET }));
        let tile_height = if large { 136.0 } else { TILE_HEIGHT };
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if let (Some(index), Some(scene)) = (artwork, self.scene.as_ref()) {
                let mut rect = screen;
                rect.max.y = (ui.cursor().top() + hero_height).min(screen.bottom());
                background.set(index, scene.fullscreen_background(rect));
            }
            if page == Page::Episodes {
                self.library_ui(ui, true);
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt(("hub", page as u8))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if page == Page::Home {
                        self.hub_hero(ui, hero_height);
                        ui.add_space(TILE_GAP);
                        ui.heading(tr("Acesso rápido"));
                    } else {
                        ui.heading(page.title());
                    }
                    if let Some((message, error)) = &self.notice {
                        ui.label(RichText::new(message).color(if *error { ERROR } else { MUTED }));
                    }
                    ui.add_space(GAP);
                    let columns = if ui.available_width() >= 1000.0 {
                        3
                    } else if ui.available_width() >= 560.0 {
                        2
                    } else {
                        1
                    };
                    let width =
                        (ui.available_width() - TILE_GAP * (columns - 1) as f32) / columns as f32;
                    if self.hub.cursor >= tiles.len() || !tiles[self.hub.cursor].enabled {
                        self.hub.cursor = tiles.iter().position(|tile| tile.enabled).unwrap_or(0);
                        self.hub.refocus = true;
                    }
                    if let Some(direction) = self.hub.direction.take() {
                        self.hub.cursor = move_cursor(&tiles, self.hub.cursor, columns, direction);
                        self.hub.refocus = true;
                    }
                    let mut column_text_widths = vec![0.0_f32; columns];
                    for (index, tile) in tiles.iter().enumerate() {
                        let galley =
                            tile_galley(ui, tile, width, large, page == Page::Home && index == 0);
                        let column = index % columns;
                        column_text_widths[column] =
                            column_text_widths[column].max(galley.size().x);
                    }
                    let confirm = std::mem::take(&mut self.hub.confirm);
                    let focus = std::mem::take(&mut self.hub.refocus);
                    ui.spacing_mut().item_spacing = Vec2::splat(TILE_GAP);
                    for (row, items) in tiles.chunks(columns).enumerate() {
                        ui.horizontal(|ui| {
                            for (column, tile) in items.iter().enumerate() {
                                let index = row * columns + column;
                                let response = ui
                                    .push_id((page as u8, index), |ui| {
                                        hub_tile(
                                            ui,
                                            tile,
                                            Vec2::new(width, tile_height),
                                            index == self.hub.cursor,
                                            large,
                                            page == Page::Home && index == 0,
                                            column_text_widths[column],
                                        )
                                    })
                                    .inner;
                                if focus && self.hub.cursor == index {
                                    response.request_focus();
                                    response.scroll_to_me(None);
                                } else if !focus && response.has_focus() {
                                    self.hub.cursor = index;
                                }
                                if tile.enabled && response.clicked() {
                                    action = Some(tile.action);
                                }
                            }
                        });
                    }
                    if action.is_none() && confirm {
                        action = tiles
                            .get(self.hub.cursor)
                            .filter(|tile| tile.enabled)
                            .map(|tile| tile.action);
                    }
                });
        });
        if let Some(action) = action {
            self.hub_action(ctx, action);
        }
    }

    fn hub_hero(&self, ui: &mut egui::Ui, height: f32) {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        let content = Rect::from_min_size(
            rect.min + Vec2::new(28.0, height * 0.22),
            Vec2::new(rect.width() * 0.50, height * 0.70),
        );
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        let ui = &mut content_ui;
        ui.set_max_width(content.width());
        ui.label(
            RichText::new(if self.loaded() {
                tr("EM REPRODUÇÃO")
            } else {
                tr("SESSÃO DE ANIME")
            })
            .color(SAKURA)
            .strong(),
        );
        let title = self.text("media-title");
        let heading = if self.loaded() && !title.is_empty() {
            title
        } else {
            Page::Home.title()
        };
        ui.add(
            egui::Label::new(
                RichText::new(heading)
                    .size(if rect.width() >= 1500.0 {
                        68.0
                    } else if rect.width() >= 900.0 {
                        48.0
                    } else {
                        33.0
                    })
                    .strong(),
            )
            .truncate(),
        );
        ui.label(
            RichText::new(if self.loaded() {
                tr("{elapsed} de {duration}")
                    .replace("{elapsed}", &super::clock(self.num("time-pos")))
                    .replace("{duration}", &super::clock(self.num("duration")))
            } else if self.ready {
                tr("Escolha um episódio para começar.").into()
            } else {
                tr("Preparando o player…").into()
            })
            .color(MUTED),
        );
    }
}

fn move_cursor(tiles: &[Tile], current: usize, columns: usize, direction: Direction) -> usize {
    let next = match direction {
        Direction::Left => (0..current).rev().find(|&i| tiles[i].enabled),
        Direction::Right => (current + 1..tiles.len()).find(|&i| tiles[i].enabled),
        Direction::Up => (0..current / columns)
            .rev()
            .find_map(|row| nearest_in_row(tiles, row, columns, current % columns)),
        Direction::Down => (current / columns + 1..tiles.len().div_ceil(columns))
            .find_map(|row| nearest_in_row(tiles, row, columns, current % columns)),
    };
    next.unwrap_or(current)
}

fn nearest_in_row(tiles: &[Tile], row: usize, columns: usize, column: usize) -> Option<usize> {
    (0..columns)
        .filter(|&i| {
            tiles
                .get(row * columns + i)
                .is_some_and(|tile| tile.enabled)
        })
        .min_by_key(|&i| i.abs_diff(column))
        .map(|i| row * columns + i)
}

fn tile_icon(action: Action) -> egui::ImageSource<'static> {
    match action {
        Action::Open { .. } => egui::include_image!("../assets/icons/folder-open.svg"),
        Action::Resume => egui::include_image!("../assets/icons/play.svg"),
        Action::Page(Page::Episodes) => egui::include_image!("../assets/icons/list-video.svg"),
        Action::Page(Page::Tracks) | Action::Track("sid", _) => {
            egui::include_image!("../assets/icons/captions.svg")
        }
        Action::Track(_, _) => egui::include_image!("../assets/icons/volume-2.svg"),
        Action::Page(Page::Motion) | Action::Motion(_) => {
            egui::include_image!("../assets/icons/gauge.svg")
        }
        Action::Page(Page::Upscale) | Action::Upscale(_) => {
            egui::include_image!("../assets/icons/scan.svg")
        }
        Action::Page(Page::Home) => egui::include_image!("../assets/icons/arrow-left.svg"),
        Action::ExitFullscreen => egui::include_image!("../assets/icons/minimize.svg"),
        Action::Quit => egui::include_image!("../assets/icons/power.svg"),
    }
}
fn tile_galley(
    ui: &egui::Ui,
    tile: &Tile,
    width: f32,
    large: bool,
    primary: bool,
) -> std::sync::Arc<egui::Galley> {
    let label = if tile.selected {
        format!("{} · {}", tile.title, tr("Ativo"))
    } else {
        tile.title.clone()
    };
    let mut text = egui::text::LayoutJob::default();
    let icon_size = if large { 36.0 } else { 28.0 };
    let icon_gap = if large { 22.0 } else { 16.0 };
    text.wrap.max_width = (width - icon_size - icon_gap - 40.0).max(1.0);
    text.wrap.max_rows = 2;
    text.append(
        &label,
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(if large { 27.0 } else { 20.0 }),
            color: if primary { SAKURA } else { INK },
            ..Default::default()
        },
    );
    text.append(
        &format!("\n{}", tile.detail),
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(if large { 18.0 } else { 14.0 }),
            color: MUTED,
            ..Default::default()
        },
    );
    ui.fonts(|fonts| fonts.layout_job(text))
}

fn hub_tile(
    ui: &mut egui::Ui,
    tile: &Tile,
    size: Vec2,
    focused: bool,
    large: bool,
    primary: bool,
    column_text_width: f32,
) -> egui::Response {
    let label = if tile.selected {
        format!("{} · {}", tile.title, tr("Ativo"))
    } else {
        tile.title.clone()
    };
    let icon_size = if large { 36.0 } else { 28.0 };
    let icon_gap = if large { 22.0 } else { 16.0 };
    let galley = tile_galley(ui, tile, size.x, large, primary);
    let button = egui::Button::new("").min_size(size).corner_radius(CORNER);
    let button = if focused && tile.enabled {
        button.fill(HOVER).stroke(Stroke::new(2.0_f32, SAKURA))
    } else {
        button.selected(tile.selected)
    };
    let response = ui.add_enabled(tile.enabled, button);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            tile.enabled,
            format!("{label}. {}", tile.detail),
        )
    });
    let content_width = icon_size + icon_gap + column_text_width;
    let content_left = response.rect.center().x - content_width * 0.5;
    let icon_rect = Rect::from_center_size(
        egui::pos2(content_left + icon_size * 0.5, response.rect.center().y),
        Vec2::splat(icon_size),
    );
    let opacity = if tile.enabled { 1.0 } else { 0.45 };
    egui::Image::new(tile_icon(tile.action))
        .fit_to_exact_size(Vec2::splat(icon_size))
        .tint(SAKURA.gamma_multiply(opacity))
        .paint_at(ui, icon_rect);
    let text_pos = egui::pos2(
        content_left + icon_size + icon_gap,
        response.rect.center().y - galley.size().y * 0.5,
    );
    let mut painter = ui.painter().clone();
    painter.multiply_opacity(opacity);
    painter.galley(text_pos, galley, INK);
    response
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_navigation_skips_disabled_and_keeps_nearest_column() {
        let tiles = vec![
            Tile::new("Resume", "", Action::Resume),
            Tile::new("Tracks", "", Action::Page(Page::Tracks)).enabled(false),
            Tile::new("Episodes", "", Action::Page(Page::Episodes)),
            Tile::new("Motion", "", Action::Page(Page::Motion)),
            Tile::new("Upscale", "", Action::Page(Page::Upscale)),
        ];
        assert_eq!(move_cursor(&tiles, 0, 3, Direction::Right), 2);
        assert_eq!(move_cursor(&tiles, 2, 3, Direction::Down), 4);
        assert_eq!(move_cursor(&tiles, 4, 3, Direction::Up), 0);
        assert_eq!(move_cursor(&tiles, 0, 3, Direction::Left), 0);
    }

    #[test]
    fn menu_returns_focus_to_originating_tile() {
        let mut hub = Hub::default();
        hub.cursor = 4;
        hub.go(Page::Upscale);
        assert_eq!(hub.cursor, 0);
        hub.go(Page::Home);
        assert_eq!(hub.cursor, 4);
        assert!(hub.refocus);
        hub.close();
        assert!(!hub.is_open());
    }
}
