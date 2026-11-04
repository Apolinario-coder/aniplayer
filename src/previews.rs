use crate::engine::{Paths, QuietCommand};
use crate::i18n::tr;
use eframe::egui::{self, Rect, TextureHandle, Vec2};
use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

const FRAME_COUNT: usize = 24;
const FRAME_SECONDS: f32 = 0.25;
const HOVER_DELAY: Duration = Duration::from_millis(600);

#[derive(Clone)]
struct Request {
    key: String,
    path: String,
    preview: bool,
}

struct Reply {
    key: String,
    preview: bool,
    images: Vec<egui::ColorImage>,
}

struct Entry {
    still: Option<TextureHandle>,
    frames: Vec<TextureHandle>,
    failed: bool,
    preview_failed: bool,
}

pub struct Previews {
    requests: SyncSender<Request>,
    results: Receiver<Reply>,
    pending: HashSet<(String, bool)>,
    entries: HashMap<String, Entry>,
    hover: Arc<Mutex<Option<String>>>,
    hovered: Option<(String, Instant)>,
    hover_frame: Option<String>,
    stop: Arc<AtomicBool>,
}

impl Previews {
    pub fn start(paths: &Paths) -> Self {
        let (requests, receiver) = mpsc::sync_channel::<Request>(24);
        let (sender, results) = mpsc::sync_channel::<Reply>(24);
        let hover = Arc::new(Mutex::new(None::<String>));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_hover = hover.clone();
        let worker_stop = stop.clone();
        let paths = paths.clone();
        thread::spawn(move || {
            let cache = paths.config.join("previews");
            while !worker_stop.load(Ordering::Relaxed) {
                let Ok(mut request) = receiver.recv_timeout(Duration::from_millis(150)) else {
                    continue;
                };
                let mut backlog = vec![request];
                // Prioritize the hovered preview between thumbnail jobs.
                while !backlog.is_empty() {
                    backlog.extend(receiver.try_iter());
                    let hovered = worker_hover.lock().ok().and_then(|key| key.clone());
                    let index = backlog
                        .iter()
                        .position(|job| job.preview && Some(&job.key) == hovered.as_ref())
                        .unwrap_or(0);
                    request = backlog.remove(index);
                    if worker_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let images = if request.preview
                        && worker_hover.lock().ok().and_then(|x| x.clone()).as_ref()
                            != Some(&request.key)
                    {
                        Vec::new()
                    } else {
                        generate(&paths, &cache, &request, &worker_stop, &worker_hover)
                            .unwrap_or_default()
                    };
                    let mut reply = Reply {
                        key: request.key,
                        preview: request.preview,
                        images,
                    };
                    loop {
                        match sender.try_send(reply) {
                            Ok(()) => break,
                            Err(mpsc::TrySendError::Full(value)) => {
                                if worker_stop.load(Ordering::Relaxed) {
                                    return;
                                }
                                reply = value;
                                thread::sleep(Duration::from_millis(50));
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => return,
                        }
                    }
                }
            }
        });
        Self {
            requests,
            results,
            pending: HashSet::new(),
            entries: HashMap::new(),
            hover,
            hovered: None,
            hover_frame: None,
            stop,
        }
    }

    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.hover_frame = None;
        for result in self.results.try_iter() {
            self.pending.remove(&(result.key.clone(), result.preview));
            let entry = self.entries.entry(result.key.clone()).or_insert(Entry {
                still: None,
                frames: Vec::new(),
                failed: false,
                preview_failed: false,
            });
            let textures: Vec<_> = result
                .images
                .into_iter()
                .enumerate()
                .map(|(index, image)| {
                    ctx.load_texture(
                        format!("episode-{}-{}-{index}", result.key, result.preview),
                        image,
                        egui::TextureOptions::LINEAR,
                    )
                })
                .collect();
            if result.preview {
                entry.preview_failed = textures.is_empty();
                entry.frames = textures;
            } else {
                entry.failed = textures.is_empty();
                entry.still = textures.into_iter().next();
            }
        }
    }

    pub fn end_frame(&mut self) {
        if self.hover_frame.is_none() {
            self.hovered = None;
            if let Ok(mut hover) = self.hover.lock() {
                *hover = None;
            }
        }
    }

    pub fn paint(&mut self, ui: &egui::Ui, path: &str, rect: Rect, hovered: bool) {
        let Some(key) = cache_key(path) else {
            ui.painter().rect_filled(rect, 5, crate::theme::CONTROL);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if path.starts_with("http") {
                    "URL"
                } else {
                    tr("Indisponível")
                },
                egui::FontId::proportional(12.0),
                crate::theme::MUTED,
            );
            return;
        };
        if !self.entries.contains_key(&key) {
            // Keep GPU memory bounded even with a very large library.
            if self.entries.len() >= 64 {
                let active = self.hovered.as_ref().map(|(key, _)| key.as_str());
                if let Some(oldest) = self
                    .entries
                    .keys()
                    .find(|key| Some(key.as_str()) != active)
                    .cloned()
                {
                    self.entries.remove(&oldest);
                }
            }
            self.entries.insert(
                key.clone(),
                Entry {
                    still: None,
                    frames: Vec::new(),
                    failed: false,
                    preview_failed: false,
                },
            );
        }
        let still_needed = self
            .entries
            .get(&key)
            .is_some_and(|entry| entry.still.is_none() && !entry.failed);
        if still_needed {
            self.request(&key, path, false);
        }
        let mut frame_index = None;
        if hovered {
            self.hover_frame = Some(key.clone());
            if self
                .hovered
                .as_ref()
                .is_none_or(|(current, _)| current != &key)
            {
                self.hovered = Some((key.clone(), Instant::now()));
                if let Ok(mut hover) = self.hover.lock() {
                    *hover = Some(key.clone());
                }
                // A cancelled preview can be generated again on a later hover.
                if let Some(entry) = self.entries.get_mut(&key) {
                    entry.preview_failed = false;
                }
            }
            let elapsed = self.hovered.as_ref().unwrap().1.elapsed();
            if elapsed >= HOVER_DELAY {
                let need = self
                    .entries
                    .get(&key)
                    .is_some_and(|e| e.frames.is_empty() && !e.preview_failed);
                if need {
                    self.request(&key, path, true);
                }
                frame_index =
                    Some(((elapsed - HOVER_DELAY).as_secs_f32() / FRAME_SECONDS) as usize);
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
        let entry = &self.entries[&key];
        let image = frame_index
            .and_then(|i| (!entry.frames.is_empty()).then(|| &entry.frames[i % entry.frames.len()]))
            .or(entry.still.as_ref());
        ui.painter().rect_filled(rect, 5, crate::theme::CONTROL);
        if let Some(texture) = image {
            egui::Image::new((
                texture.id(),
                Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32),
            ))
            .corner_radius(5)
            .paint_at(ui, rect);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if entry.failed {
                    tr("Sem prévia")
                } else {
                    "Carregando…"
                },
                egui::FontId::proportional(11.0),
                crate::theme::MUTED,
            );
        }
    }

    fn request(&mut self, key: &str, path: &str, preview: bool) {
        if self.pending.contains(&(key.into(), preview)) {
            return;
        }
        if self
            .requests
            .try_send(Request {
                key: key.into(),
                path: path.into(),
                preview,
            })
            .is_ok()
        {
            self.pending.insert((key.into(), preview));
        }
    }
}

impl Drop for Previews {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn cache_key(path: &str) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let mut hash = DefaultHasher::new();
    path.hash(&mut hash);
    metadata.len().hash(&mut hash);
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .hash(&mut hash);
    Some(format!("{:016x}", hash.finish()))
}

fn run_ffmpeg(paths: &Paths, args: &[String], log: &Path, stop: &AtomicBool) -> Result<(), String> {
    let stderr = fs::File::create(log).map_err(crate::engine::err)?;
    let mut process = Command::new(&paths.ffmpeg)
        .args(args)
        .stdout(Stdio::null())
        .stdin(Stdio::null())
        .stderr(stderr)
        .quiet()
        .spawn()
        .map_err(crate::engine::err)?;
    #[cfg(windows)]
    let _job = match crate::engine::Job::for_child(&process) {
        Ok(job) => job,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            return Err(error);
        }
    };
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(status) = process.try_wait().map_err(crate::engine::err)? {
            return if status.success() {
                Ok(())
            } else {
                Err(tr("Prévia indisponível.").into())
            };
        }
        if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
            let _ = process.kill();
            let _ = process.wait();
            return Err(tr("Prévia cancelada.").into());
        }
        thread::sleep(Duration::from_millis(30));
    }
}

fn read_image(file: &Path) -> Option<egui::ColorImage> {
    let image = image::open(file).ok()?.into_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

fn generate(
    paths: &Paths,
    cache: &Path,
    request: &Request,
    stop: &AtomicBool,
    hover: &Mutex<Option<String>>,
) -> Result<Vec<egui::ColorImage>, String> {
    let folder = cache.join(&request.key);
    fs::create_dir_all(&folder).map_err(crate::engine::err)?;
    let log = folder.join("ffmpeg.log");
    let still = folder.join("still.png");
    if !request.preview {
        if let Some(image) = read_image(&still) {
            return Ok(vec![image]);
        }
    } else {
        let images: Option<Vec<_>> = (0..FRAME_COUNT)
            .map(|i| read_image(&folder.join(format!("preview-{i:02}.png"))))
            .collect();
        if let Some(images) = images {
            return Ok(images);
        }
    }

    let duration_file = folder.join("duration.txt");
    let duration = if let Ok(value) = fs::read_to_string(&duration_file) {
        value.parse().unwrap_or(0.0)
    } else {
        let _ = run_ffmpeg(
            paths,
            &["-hide_banner".into(), "-i".into(), request.path.clone()],
            &log,
            stop,
        );
        let duration = fs::read_to_string(&log)
            .ok()
            .and_then(|text| {
                let value = text.split("Duration: ").nth(1)?.split(',').next()?;
                let mut fields = value.split(':');
                Some(
                    fields.next()?.parse::<f64>().ok()? * 3600.0
                        + fields.next()?.parse::<f64>().ok()? * 60.0
                        + fields.next()?.parse::<f64>().ok()?,
                )
            })
            .unwrap_or(0.0);
        let _ = fs::write(&duration_file, duration.to_string());
        duration
    };
    let segments = if request.preview { 3 } else { 1 };
    let seed = crate::library::nonce() as u64;
    for segment in 0..segments {
        if request.preview
            && hover.lock().ok().and_then(|x| x.clone()).as_ref() != Some(&request.key)
        {
            return Err(tr("Prévia cancelada.").into());
        }
        let fraction = if request.preview {
            0.12 + (((seed.rotate_left(segment * 13) % 10000) as f64) / 10000.0) * 0.72
        } else {
            0.20
        };
        let time = (duration * fraction).min((duration - 2.0).max(0.0));
        let output = if request.preview {
            folder.join(format!("segment-{segment}-%02d.png"))
        } else {
            still.clone()
        };
        let mut args: Vec<String> = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-hwaccel",
            "auto",
            "-ss",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        args.extend([
            time.to_string(),
            "-i".into(),
            request.path.clone(),
            "-an".into(),
            "-sn".into(),
            "-threads".into(),
            "1".into(),
            "-vf".into(),
        ]);
        args.push(if request.preview { "fps=4,scale=320:180:force_original_aspect_ratio=decrease,pad=320:180:(ow-iw)/2:(oh-ih)/2".into() }
            else { "scale=320:180:force_original_aspect_ratio=decrease,pad=320:180:(ow-iw)/2:(oh-ih)/2".into() });
        args.extend([
            "-frames:v".into(),
            if request.preview { "8" } else { "1" }.into(),
            output.to_string_lossy().into(),
        ]);
        run_ffmpeg(paths, &args, &log, stop)?;
        if request.preview {
            for frame in 0..8 {
                let source = folder.join(format!("segment-{segment}-{:02}.png", frame + 1));
                let destination =
                    folder.join(format!("preview-{:02}.png", segment as usize * 8 + frame));
                if source.is_file() {
                    fs::rename(&source, destination).map_err(crate::engine::err)?;
                }
            }
        }
    }
    let _ = fs::remove_file(log);
    let files: Vec<_> = if request.preview {
        (0..FRAME_COUNT)
            .map(|i| folder.join(format!("preview-{i:02}.png")))
            .collect()
    } else {
        vec![still]
    };
    let images: Vec<_> = files.iter().filter_map(|path| read_image(path)).collect();
    if images.is_empty() {
        Err(tr("Sem prévia.").into())
    } else {
        Ok(images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "gera thumbnails e prévias com FFmpeg local"]
    fn generates_and_reuses_still_and_random_preview_segments() {
        let mut paths = Paths::find();
        let folder =
            std::env::temp_dir().join(format!("ani-preview-test-{}", crate::library::nonce()));
        fs::create_dir(&folder).unwrap();
        paths.config = folder.clone();
        let clip = folder.join("Episódio ç.mp4");
        assert!(
            Command::new(&paths.ffmpeg)
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=640x360:rate=24",
                    "-t",
                    "6",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                ])
                .arg(&clip)
                .quiet()
                .status()
                .unwrap()
                .success()
        );
        let key = cache_key(clip.to_str().unwrap()).unwrap();
        let stop = AtomicBool::new(false);
        let hover = Mutex::new(Some(key.clone()));
        let request = Request {
            key: key.clone(),
            path: clip.to_string_lossy().into(),
            preview: false,
        };
        let still = generate(&paths, &folder.join("cache"), &request, &stop, &hover).unwrap();
        assert_eq!(still.len(), 1);
        assert_eq!(still[0].size, [320, 180]);
        let request = Request {
            preview: true,
            ..request
        };
        let frames = generate(&paths, &folder.join("cache"), &request, &stop, &hover).unwrap();
        assert_eq!(frames.len(), FRAME_COUNT);
        assert_ne!(frames[0].pixels, frames[1].pixels);
        let cached = generate(&paths, &folder.join("cache"), &request, &stop, &hover).unwrap();
        assert_eq!(cached[8].pixels, frames[8].pixels);
        // Only delete the unique, locally created test directory.
        assert!(folder.starts_with(std::env::temp_dir()));
        fs::remove_dir_all(folder).unwrap();
    }
}
