#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app_icon;
mod dock;
mod engine;
mod fullscreen;
mod i18n;
mod library;
mod library_view;
mod motion;
mod previews;
mod scene;
#[cfg(test)]
mod smoke;
mod theme;
mod updates;
mod upscale;
#[cfg(windows)]
mod video;

use crate::i18n::tr;
use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use engine::{Engine, Output, Paths, State, Update};
use motion::{Broadcast, Export, MODES, Profile};
#[cfg(windows)]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde_json::{Value, json};
use std::{
    fs,
    net::{Ipv4Addr, UdpSocket},
    path::PathBuf,
    time::{Duration, Instant},
};
use theme::*;
#[cfg(windows)]
use video::VideoSurface;

fn main() -> eframe::Result {
    let files: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let size = if std::env::var_os("ANI_PREVIEW").is_some()
        && std::env::var_os("ANI_PREVIEW_COMPACT").is_some()
    {
        [720.0, 510.0]
    } else {
        [1160.0, 735.0]
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([720.0, 510.0])
            .with_icon(app_icon::load()),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Ani Player",
        options,
        Box::new(|cc| Ok(Box::new(Player::new(cc, files)))),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Queue,
    Tracks,
    Motion,
    Tools,
    Open,
}

struct Player {
    engine: Option<Engine>,
    #[cfg(windows)]
    surface: Option<VideoSurface>,
    updates: updates::Updates,
    paths: Paths,
    library: library::Library,
    delete_folder: Option<u64>,
    previews: previews::Previews,
    scene: Option<scene::Scene>,
    state: State,
    ready: bool,
    files: Vec<PathBuf>,
    panel: Option<Tab>,
    fullscreen: bool,
    hub: fullscreen::Hub,
    dock: dock::Dock,
    on_top: bool,
    mode: String,
    applied: Option<(String, f64)>,
    profiles: Vec<Profile>,
    editing_profiles: bool,
    url: String,
    property: String,
    property_value: String,
    host: String,
    port: u16,
    notice: Option<(String, bool)>,
    export: Option<Export>,
    broadcast: Option<Broadcast>,
    stream_ready: bool,
    seek_draft: Option<f64>,
    preview: Option<(PathBuf, Instant, bool)>,
}

impl Player {
    fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let paths = Paths::find();
        i18n::load(&paths.config);
        i18n::fonts(&cc.egui_ctx);
        let mode = fs::read_to_string(paths.config.join("fluidez.txt"))
            .ok()
            .map(|x| x.trim().to_owned())
            .filter(|x| MODES.contains(&x.as_str()))
            .unwrap_or_else(|| "Original".into());
        let profiles = fs::read(paths.config.join("perfis.json"))
            .ok()
            .and_then(|x| serde_json::from_slice(&x).ok())
            .unwrap_or_default();
        #[cfg(windows)]
        let surface = match cc.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => VideoSurface::new(handle.hwnd.get()),
            _ => Err(tr("Janela do Windows indisponível.").into()),
        };
        #[cfg(windows)]
        let notice = surface.as_ref().err().map(|x| (x.clone(), true));
        #[cfg(unix)]
        let notice = None;
        #[cfg(windows)]
        let surface = surface.ok();
        #[cfg(windows)]
        let engine = surface
            .as_ref()
            .map(|window| Engine::start(paths.clone(), Output::Embedded(window.handle())));
        #[cfg(unix)]
        let engine = Some(Engine::start(paths.clone(), Output::Window));
        let host = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
            .ok()
            .and_then(|socket| {
                socket.connect((Ipv4Addr::new(192, 0, 2, 1), 80)).ok()?;
                socket.local_addr().ok().map(|x| x.ip().to_string())
            })
            .unwrap_or_else(|| "127.0.0.1".into());
        Self {
            engine,
            #[cfg(windows)]
            surface,
            updates: updates::Updates::start(),
            library: library::Library::load(&paths.config),
            delete_folder: None,
            previews: previews::Previews::start(&paths),
            paths,
            scene: scene::Scene::load(&cc.egui_ctx),
            state: State::new(),
            ready: false,
            files,
            panel: Some(Tab::Queue),
            fullscreen: false,
            hub: fullscreen::Hub::default(),
            dock: dock::Dock::default(),
            on_top: false,
            mode,
            applied: None,
            profiles,
            editing_profiles: false,
            url: String::new(),
            property: String::new(),
            property_value: String::new(),
            host,
            port: 18748,
            notice,
            export: None,
            broadcast: None,
            stream_ready: false,
            seek_draft: None,
            preview: std::env::var_os("ANI_PREVIEW")
                .map(|path| (path.into(), Instant::now(), false)),
        }
    }

    fn value(&self, key: &str) -> &Value {
        self.state.get(key).unwrap_or(&Value::Null)
    }
    fn text(&self, key: &str) -> &str {
        self.value(key).as_str().unwrap_or("")
    }
    fn num(&self, key: &str) -> f64 {
        self.value(key).as_f64().unwrap_or(0.0)
    }
    fn yes(&self, key: &str) -> bool {
        self.value(key).as_bool().unwrap_or(false)
    }
    fn loaded(&self) -> bool {
        !self.text("path").is_empty()
    }
    fn cmd(&self, args: &[&str]) {
        if let Some(engine) = &self.engine {
            engine.words(args);
        }
    }
    fn send(&self, args: Vec<Value>) {
        if let Some(engine) = &self.engine {
            engine.command(args);
        }
    }

    fn open(&mut self, paths: Vec<PathBuf>, append: bool) {
        self.library
            .add(paths.iter().map(|path| path.to_string_lossy().into_owned()));
        for (i, path) in paths.into_iter().enumerate() {
            self.send(vec![
                json!("loadfile"),
                json!(path.to_string_lossy()),
                json!(if i > 0 || append {
                    "append-play"
                } else {
                    "replace"
                }),
            ]);
        }
    }

    fn pick(&mut self, append: bool) -> bool {
        if let Some(paths) = rfd::FileDialog::new()
            .set_title(tr("Abrir mídia"))
            .add_filter(
                tr("Vídeo e áudio"),
                &[
                    "mkv", "mp4", "avi", "mov", "webm", "ts", "m2ts", "wmv", "mp3", "flac", "wav",
                    "m4a",
                ],
            )
            .add_filter(tr("Todos os arquivos"), &["*"])
            .pick_files()
        {
            self.open(paths, append);
            true
        } else {
            false
        }
    }

    fn update_state(&mut self) {
        let updates: Vec<_> = self
            .engine
            .iter()
            .flat_map(|x| x.updates.try_iter())
            .collect();
        for update in updates {
            match update {
                Update::Ready => {
                    self.ready = true;
                    if self.files.is_empty() {
                        match self.library.restore_list(&self.paths.config) {
                            Ok(Some(file)) => {
                                self.cmd(&["set", "pause", "yes"]);
                                self.send(vec![
                                    json!("loadlist"),
                                    json!(file.to_string_lossy()),
                                    json!("replace"),
                                ]);
                                let position = self
                                    .library
                                    .data
                                    .current
                                    .min(self.library.data.queue.len().saturating_sub(1));
                                self.send(vec![
                                    json!("set"),
                                    json!("playlist-pos"),
                                    json!(position.to_string()),
                                ]);
                            }
                            Ok(None) => {}
                            Err(error) => self.notice = Some((error, true)),
                        }
                    }
                    if !self.files.is_empty() {
                        let files = std::mem::take(&mut self.files);
                        self.open(files, false);
                    }
                }
                Update::Snapshot(snapshot) => {
                    self.library.sync_queue(
                        snapshot.get("playlist").unwrap_or(&Value::Null),
                        snapshot.get("playlist-pos").unwrap_or(&Value::Null),
                    );
                    self.state = snapshot;
                }
                Update::Error(message) => self.notice = Some((message, true)),
            }
        }
        if let Some(export) = &mut self.export {
            match export.poll() {
                Ok(true) => {
                    self.export = None;
                    self.notice = Some((tr("Vídeo exportado.").into(), false));
                }
                Err(error) => {
                    self.export = None;
                    self.notice = Some((error, true));
                }
                Ok(false) => {}
            }
        }
        if let Some(stream) = &mut self.broadcast {
            match stream.poll() {
                Ok(ready) => self.stream_ready = ready,
                Err(error) => {
                    self.broadcast = None;
                    self.notice = Some((error, true));
                }
            }
        }
        let mode = self.effective_mode().to_owned();
        let target = self.target(&mode);
        if self.ready
            && self
                .applied
                .as_ref()
                .is_none_or(|(current, rate)| current != &mode || (rate - target).abs() > 0.05)
        {
            let result = if target <= 0.0 {
                Ok(None)
            } else {
                motion::rife(&self.paths, (mode != "RIFE 2×").then_some(target)).map(Some)
            };
            match result {
                Ok(filter) => {
                    if let Some(engine) = &self.engine {
                        engine.motion(filter);
                    }
                }
                Err(error) => {
                    if let Some(engine) = &self.engine {
                        engine.motion(None);
                    }
                    self.notice = Some((error, true));
                }
            }
            self.applied = Some((mode, target));
        }
    }

    fn effective_mode(&self) -> &str {
        if self.mode == "Automático" {
            let fps = self.num("container-fps");
            if fps > 0.0 {
                for profile in &self.profiles {
                    if profile.matches(self.text("path"), self.num("height"), fps) {
                        return &profile.mode;
                    }
                }
            }
        }
        &self.mode
    }

    fn target(&self, mode: &str) -> f64 {
        motion::target(mode, self.num("container-fps"), self.num("display-fps"))
    }

    fn select_mode(&mut self, mode: &str) {
        self.mode = mode.into();
        self.applied = None;
        self.notice = None;
        if let Err(error) = fs::write(self.paths.config.join("fluidez.txt"), mode) {
            self.notice = Some((error.to_string(), true));
        }
    }

    fn tab(&mut self, tab: Tab) {
        self.panel = if self.panel == Some(tab) {
            None
        } else {
            Some(tab)
        };
    }

    fn fullscreen(&mut self, ctx: &egui::Context) {
        self.dock.close();
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        if self.fullscreen {
            self.show_hub();
        } else {
            #[cfg(unix)]
            self.cmd(&["set", "fullscreen", "no"]);
            if self.hub.resume_on_exit {
                self.cmd(&["set", "pause", "no"]);
            }
            self.hub.close();
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) && self.ready {
            if self.pick(false) && self.fullscreen {
                self.resume_fullscreen();
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::U)) {
            if self.fullscreen {
                self.fullscreen(ctx);
            }
            self.panel = Some(Tab::Open);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
            self.fullscreen(ctx);
        }
        if self.fullscreen {
            self.fullscreen_input(ctx);
            return;
        }
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F)) {
            self.fullscreen(ctx);
        }
        if !self.loaded() {
            return;
        }
        for (key, args) in [
            (Key::Space, &["cycle", "pause"][..]),
            (Key::M, &["cycle", "mute"]),
            (Key::ArrowLeft, &["seek", "-5", "relative"]),
            (Key::ArrowRight, &["seek", "5", "relative"]),
        ] {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                self.cmd(args);
            }
        }
    }

    fn top(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .frame(panel(PAGE))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                    ui.painter().rect_filled(rect, CORNER, SAKURA);
                    triangle(ui, rect.center(), 7.0, ON_SAKURA);
                    ui.label(RichText::new("Ani Player").size(21.0).strong());
                    ui.add_space(GAP);
                    if ui
                        .add_enabled(self.ready, primary(tr("Abrir episódio")))
                        .clicked()
                    {
                        self.pick(false);
                    }
                    if ui
                        .selectable_label(self.panel == Some(Tab::Open), tr("Abrir URL"))
                        .clicked()
                    {
                        self.tab(Tab::Open);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .selectable_label(self.panel == Some(Tab::Tools), tr("Ferramentas"))
                            .clicked()
                        {
                            self.tab(Tab::Tools);
                        }
                        if ui
                            .selectable_label(self.panel == Some(Tab::Motion), tr("Fluidez"))
                            .clicked()
                        {
                            self.tab(Tab::Motion);
                        }
                        if ui
                            .selectable_label(self.panel == Some(Tab::Tracks), tr("Faixas"))
                            .on_hover_text(tr("Áudio, legendas e capítulos"))
                            .clicked()
                        {
                            self.tab(Tab::Tracks);
                        }
                        if ui
                            .selectable_label(self.panel == Some(Tab::Queue), tr("Episódios"))
                            .clicked()
                        {
                            self.tab(Tab::Queue);
                        }
                    });
                });
            });
    }

    fn transport(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("transport")
            .default_height(110.0)
            .frame(panel(PANEL))
            .show(ctx, |ui| {
                if let Some((message, is_error)) = &self.notice {
                    let message = message.clone();
                    let color = if *is_error { ERROR } else { SAKURA };
                    ui.horizontal(|ui| {
                        if ui.small_button(tr("Dispensar")).clicked() {
                            self.notice = None;
                        }
                        ui.add(egui::Label::new(RichText::new(&message).color(color)).truncate())
                            .on_hover_text(&message);
                    });
                }
                let duration = self.num("duration");
                let position = self.num("time-pos").max(0.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(clock(position)).monospace().color(INK));
                    let width = (ui.available_width() - 130.0).max(60.0);
                    let mut seek = self.seek_draft.unwrap_or(position).min(duration.max(0.0));
                    ui.scope(|ui| {
                        ui.spacing_mut().slider_width = width;
                        let response = ui.add_enabled(
                            duration > 0.0,
                            egui::Slider::new(&mut seek, 0.0..=duration.max(0.1))
                                .show_value(false)
                                .trailing_fill(true)
                                .text(tr("Posição")),
                        );
                        if response.changed() {
                            if response.dragged() {
                                self.seek_draft = Some(seek);
                            } else {
                                self.send(vec![json!("seek"), json!(seek), json!("absolute")]);
                            }
                        }
                        if response.drag_stopped() {
                            if let Some(value) = self.seek_draft.take() {
                                self.send(vec![json!("seek"), json!(value), json!("absolute")]);
                            }
                        }
                    });
                    ui.label(RichText::new(clock(duration)).monospace().color(MUTED));
                });
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    let loaded = self.loaded();
                    if ui
                        .add_enabled(loaded, egui::Button::new(tr("Anterior")))
                        .clicked()
                    {
                        self.cmd(&["playlist-prev"]);
                    }
                    let paused = self.yes("pause") || !loaded;
                    if ui
                        .add_enabled(
                            loaded,
                            primary(if paused {
                                tr("Reproduzir")
                            } else {
                                tr("Pausar")
                            }),
                        )
                        .clicked()
                    {
                        self.cmd(&["cycle", "pause"]);
                    }
                    if ui
                        .add_enabled(loaded, egui::Button::new(tr("Próximo")))
                        .clicked()
                    {
                        self.cmd(&["playlist-next"]);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(if self.fullscreen {
                                tr("Sair da tela cheia")
                            } else {
                                tr("Tela cheia")
                            })
                            .on_hover_text("F11")
                            .clicked()
                        {
                            self.fullscreen(ctx);
                        }
                        if ui
                            .button(if self.yes("mute") {
                                tr("Ativar som")
                            } else {
                                tr("Silenciar")
                            })
                            .clicked()
                        {
                            self.cmd(&["cycle", "mute"]);
                        }
                        if ui.available_width() > 140.0 {
                            let mut volume = self.value("volume").as_f64().unwrap_or(100.0);
                            ui.scope(|ui| {
                                ui.spacing_mut().slider_width = 82.0;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut volume, 0.0..=100.0)
                                            .show_value(false)
                                            .text(tr("Volume")),
                                    )
                                    .changed()
                                {
                                    self.send(vec![
                                        json!("set"),
                                        json!("volume"),
                                        json!(volume.to_string()),
                                    ]);
                                }
                            });
                        }
                    });
                });
            });
    }

    fn fullscreen_video(&self) -> bool {
        cfg!(windows) && self.fullscreen && self.loaded() && self.num("width") > 0.0
    }

    fn stage(&mut self, ctx: &egui::Context) {
        let frame = if self.fullscreen_video() {
            egui::Frame::NONE.fill(PAGE)
        } else {
            panel(PAGE)
        };
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if self.loaded() && !self.fullscreen {
                let title = self.text("media-title");
                ui.add(
                    egui::Label::new(
                        RichText::new(if title.is_empty() {
                            tr("Reprodução")
                        } else {
                            title
                        })
                        .size(18.0)
                        .strong(),
                    )
                    .truncate(),
                );
                let fps = self.num("estimated-vf-fps");
                if fps > 0.0 {
                    ui.label(
                        RichText::new(format!(
                            "{} × {}   ·   {:.2} fps   ·   {}",
                            self.num("width"),
                            self.num("height"),
                            fps,
                            tr(self.effective_mode())
                        ))
                        .color(MUTED),
                    );
                }
            }
            let (rect, hit) = ui.allocate_exact_size(ui.available_size(), Sense::click());
            let has_video = self.num("width") > 0.0 && self.loaded();
            #[cfg(windows)]
            if let Some(surface) = &mut self.surface {
                surface.place(
                    rect,
                    ctx.screen_rect().min.to_vec2(),
                    ctx.pixels_per_point(),
                    has_video && self.delete_folder.is_none() && !self.updates.visible(),
                );
            }
            ui.painter().rect_filled(rect, CORNER, STAGE);
            if hit.double_clicked() && self.loaded() {
                self.fullscreen(ctx);
            }
            if has_video {
                #[cfg(unix)]
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    tr("O vídeo está na janela de reprodução."),
                    FontId::proportional(17.0),
                    MUTED,
                );
                return;
            }
            if self.loaded() {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    tr("Reprodução de áudio"),
                    FontId::proportional(22.0),
                    MUTED,
                );
                return;
            }
            let compact = rect.width() < 570.0;
            if let Some(scene) = &self.scene {
                scene.paint(ui, rect, compact);
            }
            let left = if compact { 18.0 } else { 32.0 };
            let top = if compact {
                rect.height() * 0.45 + 2.0
            } else {
                rect.height() * 0.28
            };
            let width = if compact {
                rect.width() - left * 2.0
            } else {
                rect.width() * 0.49
            };
            let content = Rect::from_min_size(
                rect.min + Vec2::new(left, top),
                Vec2::new(width, (rect.height() - top - 12.0).max(1.0)),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
                ui.set_max_width(width);
                ui.label(
                    RichText::new(tr("SESSÃO DE ANIME"))
                        .small()
                        .strong()
                        .color(SAKURA),
                );
                ui.add_space(if compact { 1.0 } else { 5.0 });
                ui.label(
                    RichText::new(if compact {
                        tr("Seu próximo episódio")
                    } else {
                        tr("Seu próximo episódio")
                    })
                    .size(if compact { 25.0 } else { 48.0 })
                    .strong(),
                );
                ui.label(
                    RichText::new(if self.ready {
                        if compact {
                            tr("Arraste um arquivo ou abra um episódio.")
                        } else {
                            tr("Arraste um arquivo para cá ou abra um episódio para começar.")
                        }
                    } else if self.notice.as_ref().is_some_and(|(_, error)| *error) {
                        tr("Falha ao iniciar a reprodução.")
                    } else {
                        tr("Preparando a reprodução…")
                    })
                    .color(MUTED),
                );
                ui.add_space(if compact { 2.0 } else { 7.0 });
                if ui
                    .add_enabled(self.ready, primary(tr("Escolher episódio")))
                    .clicked()
                {
                    self.pick(false);
                }
            });
        });
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.panel else { return };
        let width = if ctx.screen_rect().width() < 850.0 {
            266.0
        } else {
            316.0
        };
        egui::SidePanel::right("inspector")
            .exact_width(width)
            .resizable(false)
            .frame(panel(PANEL))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(match tab {
                        Tab::Queue => tr("Episódios"),
                        Tab::Tracks => tr("Faixas"),
                        Tab::Motion => tr("Fluidez"),
                        Tab::Tools => tr("Ferramentas"),
                        Tab::Open => tr("Abrir URL"),
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr("Fechar")).clicked() {
                            self.panel = None;
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match tab {
                        Tab::Queue => self.queue(ui),
                        Tab::Tracks => self.tracks(ui),
                        Tab::Motion => self.motion_panel(ui),
                        Tab::Tools => self.tools(ui),
                        Tab::Open => self.online(ui),
                    });
            });
    }

    fn queue(&mut self, ui: &mut egui::Ui) {
        self.library_ui(ui, false);
    }
    fn tracks(&self, ui: &mut egui::Ui) {
        if !self.loaded() {
            ui.label(RichText::new(tr("Abra um vídeo para escolher as faixas.")).color(MUTED));
            return;
        }
        for (label, kind, property) in [
            (tr("Vídeo"), "video", "vid"),
            (tr("Áudio"), "audio", "aid"),
            (tr("Legendas"), "sub", "sid"),
        ] {
            section(ui, label);
            let tracks = self.value("track-list").as_array();
            let selected = tracks.and_then(|items| {
                items
                    .iter()
                    .find(|x| x["type"] == kind && x["selected"] == true)
            });
            let selected_text = selected
                .map(track_label)
                .unwrap_or_else(|| tr("Desativada").into());
            egui::ComboBox::from_id_salt(property)
                .width(ui.available_width() - 12.0)
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(selected.is_none(), tr("Desativar faixa"))
                        .clicked()
                    {
                        self.cmd(&["set", property, "no"]);
                    }
                    if let Some(tracks) = tracks {
                        for track in tracks.iter().filter(|x| x["type"] == kind) {
                            if ui
                                .selectable_label(track["selected"] == true, track_label(track))
                                .clicked()
                            {
                                self.send(vec![
                                    json!("set"),
                                    json!(property),
                                    json!(track["id"].to_string()),
                                ]);
                            }
                        }
                    }
                });
            if kind != "video" {
                if ui
                    .button(if kind == "audio" {
                        tr("Carregar áudio externo")
                    } else {
                        tr("Carregar legenda")
                    })
                    .clicked()
                {
                    let picker = rfd::FileDialog::new();
                    let picker = if kind == "audio" {
                        picker.add_filter(
                            tr("Áudio"),
                            &["mp3", "flac", "mka", "aac", "wav", "ac3", "opus"],
                        )
                    } else {
                        picker.add_filter(
                            tr("Legendas"),
                            &["srt", "ass", "ssa", "vtt", "sup", "idx", "sub"],
                        )
                    };
                    if let Some(file) = picker.pick_file() {
                        self.send(vec![
                            json!(if kind == "audio" {
                                "audio-add"
                            } else {
                                "sub-add"
                            }),
                            json!(file.to_string_lossy()),
                            json!("select"),
                        ]);
                    }
                }
                let delay = if kind == "audio" {
                    "audio-delay"
                } else {
                    "sub-delay"
                };
                ui.horizontal(|ui| {
                    if ui.button(tr("Adiantar 100 ms")).clicked() {
                        self.cmd(&["add", delay, "-0.1"]);
                    }
                    if ui.button(tr("Atrasar 100 ms")).clicked() {
                        self.cmd(&["add", delay, "0.1"]);
                    }
                });
            }
            if kind == "sub" {
                if ui.button(tr("Mostrar / ocultar")).clicked() {
                    self.cmd(&["cycle", "sub-visibility"]);
                }
                self.choices(
                    ui,
                    tr("Tamanho"),
                    "sub-scale",
                    &["0.75", "1", "1.25", "1.5", "2"],
                );
            }
        }
        if let Some(chapters) = self
            .value("chapter-list")
            .as_array()
            .filter(|list| !list.is_empty())
        {
            section(ui, tr("Capítulos"));
            for (i, chapter) in chapters.iter().enumerate() {
                let label = chapter["title"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| tr("Capítulo {n}").replace("{n}", &(i + 1).to_string()));
                if ui
                    .selectable_label(self.value("chapter").as_u64() == Some(i as u64), label)
                    .clicked()
                {
                    self.send(vec![json!("set"), json!("chapter"), json!(i.to_string())]);
                }
            }
        }
    }

    fn motion_panel(&mut self, ui: &mut egui::Ui) {
        section(ui, "Anime4K");
        let current = upscale::Mode::current(&self.paths.config, self.value("glsl-shaders"));
        ui.add_enabled_ui(self.ready, |ui| {
            egui::ComboBox::from_id_salt("anime4k")
                .selected_text(current.label())
                .width(ui.available_width() - 12.0)
                .show_ui(ui, |ui| {
                    for mode in upscale::Mode::ALL {
                        if ui.selectable_label(mode == current, mode.label()).clicked() {
                            if let Some(engine) = &self.engine {
                                engine.upscale(mode);
                            }
                        }
                    }
                });
        });
        ui.label(RichText::new(tr("Upscaling em tempo real para GPUs de alto desempenho. A saída acompanha o tamanho do vídeo na tela.")).small().color(MUTED));
        ui.separator();
        ui.label(
            RichText::new(tr("A interpolação usa RIFE/Vulkan na GPU. As metas de FPS usam resolução interna reduzida em vídeos grandes."))
                .color(MUTED),
        );
        let original = self.num("container-fps");
        let output = self.num("estimated-vf-fps");
        ui.add_space(5.0);
        ui.columns(2, |cols| {
            for (col, name, value) in [(&mut cols[0], tr("FONTE"), original)] {
                fps_card(col, name, value);
            }
            fps_card(&mut cols[1], tr("SAÍDA"), output);
        });
        section(ui, tr("Modo de fluidez"));
        egui::ComboBox::from_id_salt("motion_mode")
            .selected_text(tr(&self.mode))
            .width(ui.available_width() - 12.0)
            .show_ui(ui, |ui| {
                for mode in MODES {
                    if ui.selectable_label(self.mode == *mode, tr(mode)).clicked() {
                        self.select_mode(mode);
                    }
                }
            });
        if self.effective_mode() != self.mode {
            ui.label(format!(
                "{} · {}",
                tr("Perfil em uso"),
                tr(self.effective_mode())
            ));
        }
        if ui
            .button(if self.editing_profiles {
                tr("Ocultar perfis")
            } else {
                tr("Editar perfis automáticos")
            })
            .clicked()
        {
            self.editing_profiles = !self.editing_profiles;
        }
        if self.editing_profiles {
            self.profile_editor(ui);
        }
        section(ui, tr("Exportação"));
        if self.export.is_some() {
            ui.label(tr("Exportando vídeo…"));
            if ui.button(tr("Cancelar exportação")).clicked() {
                self.export = None;
            }
        } else {
            let mode = self.effective_mode();
            let fps = self.target(mode);
            let can_export =
                fps > 0.0 && mode != "RIFE 2×" && std::path::Path::new(self.text("path")).is_file();
            if ui
                .add_enabled(can_export, egui::Button::new(tr("Exportar MKV")))
                .clicked()
            {
                self.export_video(fps);
            }
            if !can_export {
                ui.label(
                    RichText::new(tr(
                        "Abra um arquivo local e escolha uma taxa de saída FFmpeg.",
                    ))
                    .small()
                    .color(MUTED),
                );
            }
        }
        section(ui, tr("Transmissão HLS"));
        self.stream_panel(ui);
    }

    fn profile_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new(tr(
                "A primeira regra compatível é aplicada. Use zero para ignorar um limite.",
            ))
            .small()
            .color(MUTED),
        );
        let mut remove = None;
        for (index, profile) in self.profiles.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(tr("Regra {n}").replace("{n}", &(index + 1).to_string()));
                    if ui.button(tr("Remover")).clicked() {
                        remove = Some(index);
                    }
                });
                ui.horizontal(|ui| {
                    let label = ui.label(tr("Extensão"));
                    ui.text_edit_singleline(&mut profile.extension)
                        .labelled_by(label.id);
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Altura máx."));
                    ui.add(egui::DragValue::new(&mut profile.max_height).range(0..=16384));
                    ui.label(tr("FPS máx."));
                    ui.add(egui::DragValue::new(&mut profile.max_fps).range(0.0..=500.0));
                });
                egui::ComboBox::from_id_salt("mode")
                    .selected_text(tr(&profile.mode))
                    .show_ui(ui, |ui| {
                        for mode in MODES.iter().filter(|mode| **mode != "Automático") {
                            ui.selectable_value(&mut profile.mode, (*mode).to_owned(), tr(mode));
                        }
                    });
            });
        }
        if let Some(index) = remove {
            self.profiles.remove(index);
        }
        ui.horizontal(|ui| {
            if ui.button(tr("Adicionar regra")).clicked() {
                self.profiles.push(Profile {
                    mode: "Original".into(),
                    ..Profile::default()
                });
            }
            if ui.button(tr("Salvar perfis")).clicked() {
                let result = serde_json::to_vec_pretty(&self.profiles)
                    .map_err(engine::err)
                    .and_then(|bytes| {
                        fs::write(self.paths.config.join("perfis.json"), bytes).map_err(engine::err)
                    });
                self.notice = Some(match result {
                    Ok(()) => (tr("Perfis salvos.").into(), false),
                    Err(error) => (error, true),
                });
            }
        });
    }

    fn export_video(&mut self, fps: f64) {
        let source = PathBuf::from(self.text("path"));
        let stem = source
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or(tr("Vídeo"));
        if let Some(destination) = rfd::FileDialog::new()
            .add_filter("Matroska", &["mkv"])
            .set_file_name(format!("{stem} - Ani.mkv"))
            .save_file()
        {
            match Export::start(&self.paths, &source, destination, fps) {
                Ok(export) => {
                    self.export = Some(export);
                    self.notice = None;
                }
                Err(error) => self.notice = Some((error, true)),
            }
        }
    }

    fn stream_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(stream) = &self.broadcast {
            if self.stream_ready {
                ui.label(RichText::new(&stream.url).monospace().color(SAKURA));
                if ui.button(tr("Copiar link")).clicked() {
                    ui.ctx().copy_text(stream.url.clone());
                }
            } else {
                ui.label(tr("Aguardando os primeiros segmentos…"));
            }
            if ui.button(tr("Encerrar transmissão")).clicked() {
                self.broadcast = None;
            }
            return;
        }
        let source = PathBuf::from(self.text("path"));
        let mode = self.effective_mode().to_owned();
        let can_stream = source.is_file() && mode != "RIFE 2×";
        ui.label(
            RichText::new(tr(
                "Pessoas com acesso ao endereço escolhido poderão abrir o vídeo.",
            ))
            .small()
            .color(MUTED),
        );
        ui.horizontal(|ui| {
            let label = ui.label("IPv4");
            ui.add(egui::TextEdit::singleline(&mut self.host).desired_width(112.0))
                .labelled_by(label.id);
            ui.add(
                egui::DragValue::new(&mut self.port)
                    .range(1024..=65535)
                    .prefix(tr("Porta")),
            );
        });
        if ui
            .add_enabled(can_stream, egui::Button::new(tr("Iniciar transmissão")))
            .clicked()
        {
            let result = self
                .host
                .trim()
                .parse::<Ipv4Addr>()
                .map_err(|_| tr("Informe um IPv4 válido.").to_owned())
                .and_then(|host| {
                    if host.is_unspecified() {
                        return Err(tr("Escolha uma interface IPv4 específica.").into());
                    }
                    Broadcast::start(&self.paths, &source, self.target(&mode), host, self.port)
                });
            match result {
                Ok(broadcast) => {
                    self.broadcast = Some(broadcast);
                    self.stream_ready = false;
                }
                Err(error) => self.notice = Some((error, true)),
            }
        }
        if !can_stream {
            ui.label(
                RichText::new(tr(
                    "Disponível para arquivos locais em modo Original ou FFmpeg.",
                ))
                .small()
                .color(MUTED),
            );
        }
    }

    fn online(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr("Cole um link HTTP ou HTTPS.")).color(MUTED));
        let label = ui.label(tr("Endereço do vídeo"));
        let input = ui
            .add(
                egui::TextEdit::singleline(&mut self.url)
                    .hint_text("https://")
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label.id);
        let submitted = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui
            .add_enabled(
                self.ready && !self.url.trim().is_empty(),
                primary(tr("Reproduzir URL")),
            )
            .clicked()
            || submitted
        {
            let url = self.url.trim();
            if url.split_once("://").is_some_and(|(scheme, rest)| {
                matches!(scheme, "http" | "https")
                    && !rest.is_empty()
                    && !rest.chars().any(char::is_whitespace)
            }) {
                self.send(vec![json!("loadfile"), json!(url), json!("replace")]);
                self.panel = Some(Tab::Queue);
                self.notice = None;
            } else {
                self.notice = Some((tr("Informe um endereço HTTP ou HTTPS válido.").into(), true));
            }
        }
        section(ui, tr("Qualidade online"));
        for (label, format) in [
            (tr("Melhor disponível"), "bestvideo+bestaudio/best"),
            (
                tr("Até 1080p"),
                "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
            ),
            (
                tr("Até 720p"),
                "bestvideo[height<=720]+bestaudio/best[height<=720]",
            ),
        ] {
            if ui.button(label).clicked() {
                self.cmd(&["set", "ytdl-format", format]);
            }
        }
    }

    fn choices(&self, ui: &mut egui::Ui, label: &str, property: &str, options: &[&str]) {
        ui.menu_button(label, |ui| {
            for option in options {
                if ui.button(*option).clicked() {
                    self.cmd(&["set", property, option]);
                    ui.close_menu();
                }
            }
        });
    }

    fn tools(&mut self, ui: &mut egui::Ui) {
        section(ui, tr("Reprodução"));
        ui.horizontal_wrapped(|ui| {
            for (label, args) in [
                (tr("Parar"), &["stop"][..]),
                (
                    tr("Repetir vídeo"),
                    &["cycle-values", "loop-file", "inf", "no"],
                ),
                (
                    tr("Repetir fila"),
                    &["cycle-values", "loop-playlist", "inf", "no"],
                ),
                (tr("Marcar trecho A–B"), &["ab-loop"]),
                (tr("Quadro anterior"), &["frame-back-step"]),
                (tr("Próximo quadro"), &["frame-step"]),
            ] {
                if ui.button(label).clicked() {
                    self.cmd(args);
                }
            }
        });
        self.choices(
            ui,
            tr("Velocidade"),
            "speed",
            &["0.5", "0.75", "1", "1.25", "1.5", "2"],
        );
        let on_top = ui.checkbox(&mut self.on_top, tr("Sempre visível"));
        if on_top.changed() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                }));
        }
        section(ui, tr("Imagem"));
        let fill = self.num("panscan") >= 0.5;
        ui.horizontal(|ui| {
            if ui.selectable_label(fill, tr("Preencher")).clicked() {
                self.cmd(&["set", "panscan", "1"]);
            }
            if ui.selectable_label(!fill, tr("Ajustar")).clicked() {
                self.cmd(&["set", "panscan", "0"]);
            }
        });
        ui.horizontal_wrapped(|ui| {
            self.choices(
                ui,
                tr("Rotação"),
                "video-rotate",
                &["0", "90", "180", "270"],
            );
            self.choices(
                ui,
                tr("Proporção"),
                "video-aspect-override",
                &["no", "4:3", "16:9", "21:9"],
            );
            self.choices(ui, tr("Decodificação"), "hwdec", &["auto-copy", "no"]);
            self.choices(
                ui,
                tr("Zoom"),
                "video-zoom",
                &["-0.25", "0", "0.25", "0.5", "1"],
            );
            self.choices(
                ui,
                tr("Desentrelaçar"),
                "deinterlace",
                &["auto", "yes", "no"],
            );
            for (label, property) in [
                (tr("Brilho"), "brightness"),
                (tr("Contraste"), "contrast"),
                (tr("Saturação"), "saturation"),
            ] {
                self.choices(ui, label, property, &["-20", "-10", "0", "10", "20"]);
            }
            self.choices(
                ui,
                "HDR",
                "tone-mapping",
                &["auto", "bt.2390", "bt.2446a", "reinhard", "mobius"],
            );
        });
        if ui
            .add_enabled(
                self.num("width") > 0.0,
                egui::Button::new(tr("Salvar captura")),
            )
            .clicked()
        {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name("ani-player.png")
                .save_file()
            {
                self.send(vec![
                    json!("screenshot-to-file"),
                    json!(file.to_string_lossy()),
                    json!("subtitles"),
                ]);
            }
        }
        if self.loaded() {
            section(ui, tr("Informações da mídia"));
            ui.label(format!("{} · {}", tr("Vídeo"), self.text("video-codec")));
            ui.label(format!(
                "{} · {}",
                tr("Áudio"),
                self.text("audio-codec-name")
            ));
            ui.label(format!(
                "{} · {:.2} fps",
                tr("Taxa de origem"),
                self.num("container-fps")
            ));
        }
        section(ui, tr("Configuração"));
        i18n::menu(ui, &self.paths.config);
        ui.collapsing(tr("Propriedade avançada do mpv"), |ui| {
            let property = ui.label(tr("Propriedade"));
            ui.text_edit_singleline(&mut self.property)
                .labelled_by(property.id);
            let value = ui.label(tr("Valor"));
            ui.text_edit_singleline(&mut self.property_value)
                .labelled_by(value.id);
            if ui.button(tr("Aplicar propriedade")).clicked() && !self.property.trim().is_empty() {
                self.cmd(&["set", self.property.trim(), &self.property_value]);
            }
        });
        if ui.button(tr("Abrir pasta de configuração")).clicked() {
            let opener = if cfg!(windows) {
                "explorer.exe"
            } else if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            if let Err(error) = std::process::Command::new(opener)
                .arg(&self.paths.config)
                .spawn()
            {
                self.notice = Some((error.to_string(), true));
            }
        }
        ui.collapsing(tr("Sobre o Ani Player"), |ui| {
            ui.label(format!("Ani Player {}", env!("CARGO_PKG_VERSION")));
            ui.label(tr(
                "Interface e controle em Rust. Motor mpv, interpolação FFmpeg e RIFE.",
            ));
            ui.label(tr(
                "Ctrl+O abre arquivos, Ctrl+U abre URL e F11 alterna a tela cheia.",
            ));
        });
    }
}

impl eframe::App for Player {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.previews.begin_frame(ctx);
        self.updates.poll();
        self.update_state();
        if self
            .preview
            .as_ref()
            .is_some_and(|(_, _, requested)| !requested)
            && !self.fullscreen
            && std::env::var_os("ANI_PREVIEW_FULLSCREEN").is_some()
        {
            self.fullscreen(ctx);
        }
        if self.delete_folder.is_none() && !self.updates.visible() {
            self.shortcuts(ctx);
        }
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if self.delete_folder.is_none() && self.ready && !dropped.is_empty() {
            self.open(dropped, false);
            if self.fullscreen {
                self.resume_fullscreen();
            }
        }
        if self.fullscreen && self.hub.is_open() {
            self.fullscreen_hub(ctx);
        } else {
            if !self.fullscreen {
                self.top(ctx);
            }
            if !self.fullscreen_video() {
                self.transport(ctx);
            }
            if !self.fullscreen {
                self.inspector(ctx);
            }
            if self.fullscreen {
                self.fullscreen_settings(ctx);
            }
            self.stage(ctx);
        }
        self.folder_delete_modal(ctx);
        self.updates.show(ctx);
        if let Some((path, started, requested)) = &mut self.preview {
            if let Some(image) = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            }) {
                let pixels = image.as_raw();
                let _ = image::save_buffer(
                    path,
                    pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if !*requested
                && started.elapsed()
                    > Duration::from_secs(if std::env::var_os("ANI_PREVIEW_LIBRARY").is_some() {
                        5
                    } else {
                        1
                    })
            {
                *requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        self.previews.end_frame();
        ctx.request_repaint_after(Duration::from_millis(if self.fullscreen {
            32
        } else {
            115
        }));
    }
}

fn fps_card(ui: &mut egui::Ui, name: &str, value: f64) {
    egui::Frame::NONE
        .fill(STAGE)
        .corner_radius(CORNER)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 8.0).max(54.0));
            ui.label(RichText::new(name).small().strong().color(MUTED));
            ui.label(
                RichText::new(if value > 0.0 {
                    format!("{value:.2}")
                } else {
                    "·".into()
                })
                .size(23.0)
                .strong()
                .color(SAKURA),
            );
            ui.label(RichText::new("fps").small().color(MUTED));
        });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(16.0).strong());
}

fn track_label(track: &Value) -> String {
    let label = ["title", "lang", "codec"]
        .iter()
        .find_map(|key| track[*key].as_str().filter(|x| !x.is_empty()))
        .unwrap_or(tr("Faixa"));
    format!("{} · {}", track["id"], label)
}

fn clock(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

fn triangle(ui: &egui::Ui, center: Pos2, radius: f32, color: Color32) {
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-radius * 0.65, -radius),
            center + Vec2::new(radius, 0.0),
            center + Vec2::new(-radius * 0.65, radius),
        ],
        color,
        Stroke::NONE,
    ));
}
