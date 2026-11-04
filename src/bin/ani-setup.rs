#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

use crate::i18n::tr;
#[path = "../app_icon.rs"]
mod app_icon;
#[path = "../i18n.rs"]
mod i18n;
#[path = "../scene.rs"]
mod scene;
#[path = "../theme.rs"]
mod theme;

use eframe::egui::{self, Align, Layout, RichText, Vec2};
use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use theme::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ACTION_HEIGHT: f32 = 48.0;
const NO_WINDOW: u32 = 0x08000000;

enum Step {
    Ready,
    Installing(Receiver<InstallEvent>),
    Done,
    Error(String),
}

enum InstallEvent {
    Progress(u8),
    Finished(Result<(), String>),
}

struct Setup {
    art: Option<scene::Scene>,
    step: Step,
    desktop: bool,
    parent: PathBuf,
    progress: u8,
    preview: Option<PathBuf>,
}

fn install_parent() -> PathBuf {
    let local = PathBuf::from(env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA"));
    fs::read_to_string(local.join("Ani Player Anime/install-location.txt"))
        .ok()
        .map(PathBuf::from)
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| local.join("Programs"))
}

fn main() -> eframe::Result {
    i18n::load(&i18n::config_dir());
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() == Some("--install".as_ref()) {
        let parent = args
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(install_parent);
        std::process::exit(if install(&parent, false, |_| {}).is_ok() {
            0
        } else {
            1
        });
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([790.0, 650.0])
            .with_min_inner_size([420.0, 450.0])
            .with_icon(app_icon::load()),
        ..Default::default()
    };
    eframe::run_native(
        tr("Instalador do Ani Player"),
        options,
        Box::new(|cc| {
            install_theme(&cc.egui_ctx);
            i18n::fonts(&cc.egui_ctx);
            let preview = env::var_os("ANI_SETUP_PREVIEW").map(PathBuf::from);
            let step = if preview.is_some() {
                match env::var("ANI_SETUP_STATE").as_deref() {
                    Ok("done") => Step::Done,
                    Ok("error") => Step::Error(tr("Feche o Ani Player e tente novamente.").into()),
                    _ => Step::Ready,
                }
            } else {
                Step::Ready
            };
            Ok(Box::new(Setup {
                art: scene::Scene::load(&cc.egui_ctx),
                step,
                desktop: preview.is_some() && env::var_os("ANI_SETUP_SELECTED").is_some(),
                parent: install_parent(),
                progress: 0,
                preview,
            }))
        }),
    )
}

fn install_theme(ctx: &egui::Context) {
    theme::install(ctx);
    ctx.style_mut(|style| {
        style.spacing.interact_size.y = ACTION_HEIGHT;
        style.visuals.widgets.noninteractive.bg_fill = CONTROL;
        style.visuals.selection.stroke.width = 2.0;
    });
}

impl Setup {
    fn start(&mut self, ctx: &egui::Context) {
        let (send, receive) = mpsc::channel();
        let desktop = self.desktop;
        let parent = self.parent.clone();
        let ctx = ctx.clone();
        self.progress = 0;
        self.step = Step::Installing(receive);
        thread::spawn(move || {
            let result = install(&parent, desktop, |progress| {
                let _ = send.send(InstallEvent::Progress(progress));
                ctx.request_repaint();
            });
            let _ = send.send(InstallEvent::Finished(
                result.map_err(|error| error.to_string()),
            ));
            ctx.request_repaint();
        });
    }
}

impl eframe::App for Setup {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Step::Installing(worker) = &self.step {
            loop {
                match worker.try_recv() {
                    Ok(InstallEvent::Progress(progress)) => self.progress = progress,
                    Ok(InstallEvent::Finished(Ok(()))) => {
                        self.progress = 100;
                        self.step = Step::Done;
                        break;
                    }
                    Ok(InstallEvent::Finished(Err(error))) => {
                        self.step = Step::Error(error);
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.step = Step::Error(tr("A instalação foi interrompida.").into());
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        if ctx.input(|input| input.viewport().close_requested()) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                        }
                        ctx.request_repaint_after(Duration::from_millis(100));
                        break;
                    }
                }
            }
        }
        let busy = matches!(self.step, Step::Installing(_));
        let done = matches!(self.step, Step::Done);
        let failed = matches!(self.step, Step::Error(_));
        let preview_progress =
            self.preview.is_some() && env::var("ANI_SETUP_STATE").as_deref() == Ok("progress");
        let busy = busy || preview_progress;
        let progress = if preview_progress { 62 } else { self.progress };

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(PAGE))
            .show(ctx, |ui| {
                let screen = ui.max_rect();
                if let Some(art) = &self.art {
                    let hero = egui::Rect::from_min_size(
                        screen.min,
                        Vec2::new(screen.width(), (screen.height() * 0.49).clamp(220.0, 350.0)),
                    );
                    ui.painter().add(art.fullscreen_background(hero));
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let inset = if screen.width() < 550.0 { 20 } else { 36 };
                    egui::Frame::NONE.inner_margin(egui::Margin::same(inset)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("ANI PLAYER   /   INSTALAÇÃO")).color(SAKURA).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if i18n::menu(ui, &i18n::config_dir()) {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(tr("Instalador do Ani Player").into()));
                                }
                            });
                        });
                        ui.add_space(if screen.height() < 500.0 { 18.0 } else { 30.0 });
                        ui.heading(RichText::new("Ani Player").size(54.0).color(INK));
                        ui.label(RichText::new(tr("Seu player de anime para Windows")).size(17.0).color(MUTED));
                        ui.add_space((screen.height() * 0.12).clamp(28.0, 76.0));
                        egui::Frame::NONE
                            .fill(PANEL)
                            .stroke(egui::Stroke::new(1.0_f32, BORDER))
                            .corner_radius(CORNER)
                            .inner_margin(egui::Margin::same(20))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.heading(if done {
                                    tr("Tudo pronto")
                                } else if busy {
                                    tr("Instalando o Ani Player…")
                                } else if failed {
                                    tr("Instalação interrompida")
                                } else {
                                    tr("Instalar o Ani Player")
                                });
                                if let Step::Error(error) = &self.step {
                                    ui.label(RichText::new(error).color(ERROR));
                                } else {
                                    ui.label(RichText::new(if done {
                                        tr("Abra o player agora ou encontre-o no menu Iniciar.")
                                    } else if busy {
                                        match progress {
                                            0..=10 => tr("Preparando o pacote de instalação…"),
                                            11..=89 => tr("Extraindo o player e os componentes de vídeo…"),
                                            _ => tr("Configurando os atalhos e a desinstalação…"),
                                        }
                                    } else {
                                        tr("Inclui reprodução, Anime4K e interpolação RIFE.")
                                    }).color(MUTED));
                                }
                                ui.add_space(GAP);
                                if busy || done {
                                    let progress = if done { 100 } else { progress };
                                    ui.add(egui::ProgressBar::new(f32::from(progress) / 100.0)
                                        .desired_height(16.0)
                                        .fill(SAKURA));
                                    ui.label(RichText::new(tr("{n}% concluído").replace("{n}", &progress.to_string())).color(MUTED));
                                } else if !done {
                                    self.folder_control(ui);
                                    let checkbox = ui.checkbox(&mut self.desktop, tr("Criar atalho na área de trabalho"));
                                    focus_ring(ui, &checkbox);
                                }
                                ui.add_space(GAP);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(ui.available_width(), ACTION_HEIGHT),
                                    Layout::right_to_left(Align::Center),
                                    |ui| {
                                    let label = if busy { tr("Instalando…") } else if done { tr("Abrir Ani Player") } else if failed {
                                        tr("Tentar novamente")
                                    } else { tr("Instalar") };
                                    let action = ui.add_enabled(!busy, primary(label));
                                    focus_ring(ui, &action);
                                    if action.clicked() {
                                        if done {
                                            let folder = self.parent.join("Ani Player");
                                            match Command::new(folder.join("Ani Player.exe")).current_dir(folder).spawn() {
                                                Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                                                Err(error) => self.step = Step::Error(format!(
                                                    "{} {error}", tr("Não foi possível abrir o Ani Player.")
                                                )),
                                            }
                                        } else {
                                            self.start(ctx);
                                        }
                                    }
                                    let secondary = ui.add_enabled(!busy, egui::Button::new(if done || failed {
                                        tr("Fechar")
                                    } else { tr("Cancelar") }));
                                    focus_ring(ui, &secondary);
                                    if secondary.clicked() {
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                });
                            });
                        ui.add_space(GAP);
                        ui.label(RichText::new(format!("{} {VERSION}  ·  WINDOWS X64", tr("VERSÃO"))).small().color(MUTED));
                    });
                });
            });

        if let Some(path) = &self.preview {
            if let Some(image) = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            }) {
                let _ = image::save_buffer(
                    path,
                    image.as_raw(),
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
    }
}

impl Setup {
    fn folder_control(&mut self, ui: &mut egui::Ui) {
        let label = ui.label(RichText::new(tr("Pasta de instalação")).strong());
        ui.horizontal(|ui| {
            let button_width = 116.0;
            let field_width = (ui.available_width() - button_width - GAP).max(1.0);
            let path = self.parent.join("Ani Player").display().to_string();
            ui.add_sized(
                [field_width, ACTION_HEIGHT],
                egui::TextEdit::singleline(&mut path.as_str())
                    .interactive(false)
                    .vertical_align(Align::Center)
                    .margin(egui::Margin::symmetric(10, 2)),
            )
            .labelled_by(label.id)
            .on_hover_text(&path);
            let browse = ui.add_sized(
                [button_width, ACTION_HEIGHT],
                egui::Button::new(tr("Alterar…")),
            );
            focus_ring(ui, &browse);
            if browse.clicked() {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title(tr("Escolha onde criar a pasta Ani Player"))
                    .set_directory(&self.parent)
                    .pick_folder()
                {
                    self.parent = folder;
                }
            }
        });
    }
}

fn focus_ring(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            CORNER,
            egui::Stroke::new(2.0_f32, SAKURA),
            egui::StrokeKind::Outside,
        );
    }
}

fn install(
    parent: &Path,
    desktop: bool,
    progress: impl Fn(u8),
) -> Result<(), Box<dyn std::error::Error>> {
    let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = env::temp_dir().join(format!("ani-setup-{}-{id}", std::process::id()));
    fs::create_dir(&temp)?;
    let result = deploy(&temp, parent, desktop, progress);
    let _ = fs::remove_dir_all(temp);
    result
}

fn deploy(
    temp: &Path,
    parent: &Path,
    desktop: bool,
    progress: impl Fn(u8),
) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = fs::File::open(env::current_exe()?)?;
    let end = source.seek(SeekFrom::End(-16))?;
    let mut footer = [0; 16];
    source.read_exact(&mut footer)?;
    let size = u64::from_le_bytes(footer[..8].try_into()?);
    if &footer[8..] != b"ANISETUP" || size == 0 || size >= end {
        return Err(tr("Pacote inválido. Abra uma cópia completa do instalador.").into());
    }
    source.seek(SeekFrom::Start(end - size))?;
    let archive = temp.join("payload.zip");
    io::copy(&mut source.take(size), &mut fs::File::create(&archive)?)?;
    let script = temp.join("deploy.ps1");
    let mut contents = vec![0xef, 0xbb, 0xbf];
    contents.extend_from_slice(include_bytes!("../../installer/Deploy.ps1"));
    fs::write(&script, contents)?;
    progress(10);
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-Archive")
        .arg(archive)
        .arg("-Version")
        .arg(VERSION)
        .args(if desktop { &["-Desktop"][..] } else { &[] })
        .env("ANI_SETUP_PARENT", parent)
        .creation_flags(NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    for line in BufReader::new(child.stdout.take().expect("piped stdout")).lines() {
        if let Ok(value) = line?.trim().parse::<u8>() {
            progress(value.min(99));
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} {}",
            tr("Não foi possível instalar."),
            tr(message.trim())
        )
        .into());
    }
    Ok(())
}
