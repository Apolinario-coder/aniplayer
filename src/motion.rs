use crate::engine::{Paths, QuietCommand, err};
use crate::i18n::tr;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MODES: &[&str] = &[
    "Original",
    "Automático",
    "Taxa da tela",
    "2×",
    "2,5×",
    "3×",
    "4×",
    "5×",
    "48 fps",
    "50 fps",
    "60 fps",
    "72 fps",
    "90 fps",
    "120 fps",
    "RIFE 2×",
];

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Profile {
    pub extension: String,
    pub max_height: u32,
    pub max_fps: f64,
    pub mode: String,
}

impl Profile {
    pub fn matches(&self, path: &str, height: f64, fps: f64) -> bool {
        let extension = path.split('?').next().unwrap_or(path);
        let extension = Path::new(extension)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("");
        (self.extension.trim().is_empty()
            || self
                .extension
                .trim()
                .trim_start_matches('.')
                .eq_ignore_ascii_case(extension))
            && (self.max_height == 0 || height <= f64::from(self.max_height))
            && (self.max_fps <= 0.0 || fps <= self.max_fps)
            && self.mode != "Automático"
            && MODES.contains(&self.mode.as_str())
    }
}

pub fn target(mode: &str, fps: f64, display: f64) -> f64 {
    if fps <= 0.0 {
        return 0.0;
    }
    match mode {
        "Original" => 0.0,
        "Automático" => {
            if fps < 59.0 {
                60.0
            } else {
                0.0
            }
        }
        "Taxa da tela" => {
            if display > fps + 0.5 {
                display
            } else {
                0.0
            }
        }
        "RIFE 2×" => fps * 2.0,
        mode if mode.ends_with('×') => {
            mode.trim_end_matches('×')
                .replace(',', ".")
                .parse::<f64>()
                .unwrap_or(0.0)
                * fps
        }
        mode => mode
            .split_whitespace()
            .next()
            .and_then(|x| x.parse::<f64>().ok())
            .filter(|x| *x > fps)
            .unwrap_or(0.0),
    }
}

pub fn filter(fps: f64) -> String {
    format!("minterpolate=fps={fps:.3}:mi_mode=mci:mc_mode=aobmc:me_mode=bidir:vsbmc=1:scd=fdiff")
}

pub fn rife(paths: &Paths, target: Option<f64>) -> Result<String, String> {
    let root = std::env::var_os("ANI_RIFE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                paths.mpv.parent().unwrap_or(Path::new(".")).join("../rife")
            } else {
                std::env::var_os("ANI_MEDIA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| paths.config.clone())
                    .join("rife")
            }
        });
    let plugin = root.join(if cfg!(windows) {
        "rife_vs.dll"
    } else if cfg!(target_os = "macos") {
        "librife.dylib"
    } else {
        "librife.so"
    });
    let model = root.join("models/rife-v4.16lite");
    if !plugin.is_file() || !model.join("flownet.bin").is_file() {
        return Err(tr("RIFE requer VapourSynth e o modelo 4.16 Lite.").into());
    }
    let (name, rate) = if let Some(fps) = target {
        let milli = (fps * 1000.0).round() as u32;
        (
            format!("ani-rife-{milli}.vpy"),
            format!("fps_num={milli}, fps_den=1000"),
        )
    } else {
        (
            "ani-rife-2x.vpy".into(),
            "factor_num=2, factor_den=1".into(),
        )
    };
    let script = paths.config.join(name);
    let plugin = serde_json::to_string(&plugin.to_string_lossy()).map_err(err)?;
    let model = serde_json::to_string(&model.to_string_lossy()).map_err(err)?;
    fs::write(
        &script,
        format!(
            r#"import vapoursynth as vs
from fractions import Fraction

core = vs.core
rate = dict({rate})
if 'fps_num' in rate and not (0 < container_fps < rate['fps_num'] / rate['fps_den']):
    video_in.set_output()
else:
    core.std.LoadPlugin({plugin})
    width, height = video_in.width, video_in.height
    if 'fps_num' in rate:
        factor = (Fraction(rate['fps_num'], rate['fps_den']) / Fraction(container_fps).limit_denominator(1001)).limit_denominator(10000)
        rate = dict(factor_num=factor.numerator, factor_den=factor.denominator)
        scale = min(1, 960 / width, 540 / height)
        if scale < 1:
            width, height = max(2, round(width * scale / 2) * 2), max(2, round(height * scale / 2) * 2)
    rgb = video_in.resize.Bicubic(width=width, height=height, format=vs.RGBS, matrix_in_s='709')
    result = core.rife.RIFE(rgb, model_path={model}, **rate)
    result.resize.Bicubic(width=video_in.width, height=video_in.height, format=video_in.format.id, matrix_s='709').set_output()
"#
        ),
    )
    .map_err(err)?;
    let script = script.to_string_lossy();
    Ok(format!(
        "vapoursynth=file=%{}%{script}:concurrent-frames=4",
        script.len()
    ))
}

fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub struct Export {
    encoder: Process,
    pending: PathBuf,
    destination: PathBuf,
    log: PathBuf,
}

impl Export {
    pub fn start(
        paths: &Paths,
        source: &Path,
        destination: PathBuf,
        fps: f64,
    ) -> Result<Self, String> {
        if let (Ok(from), Ok(to)) = (source.canonicalize(), destination.canonicalize()) {
            if from
                .to_string_lossy()
                .eq_ignore_ascii_case(&to.to_string_lossy())
            {
                return Err(tr("Escolha um destino diferente do arquivo original.").into());
            }
        }
        let folder = destination.parent().ok_or(tr("Destino inválido."))?;
        let pending = folder.join(format!(".ani-{}.mkv", stamp()));
        let log = pending.with_extension("log");
        let stderr = fs::File::create(&log).map_err(err)?;
        let encoder = Command::new(&paths.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
            .arg(source)
            .args(["-map", "0:v:0", "-map", "0:a?", "-map", "0:s?", "-vf"])
            .arg(filter(fps))
            .args([
                "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-c:a", "copy", "-c:s", "copy",
            ])
            .arg(&pending)
            .stderr(stderr)
            .quiet()
            .spawn();
        match encoder {
            Ok(encoder) => Ok(Self {
                encoder: Process(encoder),
                pending,
                destination,
                log,
            }),
            Err(error) => {
                let _ = fs::remove_file(&log);
                Err(err(error))
            }
        }
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        let Some(status) = self.encoder.0.try_wait().map_err(err)? else {
            return Ok(false);
        };
        if !status.success() {
            return Err(
                fs::read_to_string(&self.log).unwrap_or_else(|_| tr("Falha na exportação.").into())
            );
        }
        fs::rename(&self.pending, &self.destination).map_err(err)?;
        Ok(true)
    }
}

impl Drop for Export {
    fn drop(&mut self) {
        let _ = self.encoder.0.kill();
        let _ = self.encoder.0.wait();
        let _ = fs::remove_file(&self.pending);
        let _ = fs::remove_file(&self.log);
    }
}

pub struct Broadcast {
    encoder: Process,
    server: Process,
    directory: PathBuf,
    pub url: String,
}

impl Broadcast {
    pub fn start(
        paths: &Paths,
        source: &Path,
        fps: f64,
        address: Ipv4Addr,
        port: u16,
    ) -> Result<Self, String> {
        let python = paths.python();
        let directory = std::env::temp_dir().join(format!("ani-rust-hls-{}", stamp()));
        fs::create_dir(&directory).map_err(err)?;
        let start = || -> Result<Self, String> {
            fs::write(directory.join("server.py"), include_str!("hls.py")).map_err(err)?;
            let server_log = fs::File::create(directory.join("server.log")).map_err(err)?;
            let server = Process(
                Command::new(python)
                    .args(["-I", "server.py", &address.to_string(), &port.to_string()])
                    .current_dir(&directory)
                    .stderr(server_log)
                    .quiet()
                    .spawn()
                    .map_err(err)?,
            );
            let ffmpeg_log = fs::File::create(directory.join("ffmpeg.log")).map_err(err)?;
            let scale = "scale=trunc(iw/2)*2:trunc(ih/2)*2";
            let video_filter = if fps > 0.0 {
                format!("{},{scale}", filter(fps))
            } else {
                scale.into()
            };
            let encoder = Process(
                Command::new(&paths.ffmpeg)
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-nostdin",
                        "-re",
                        "-i",
                    ])
                    .arg(source)
                    .args([
                        "-map",
                        "0:v:0",
                        "-map",
                        "0:a:0?",
                        "-sn",
                        "-vf",
                        &video_filter,
                        "-c:v",
                        "libx264",
                        "-pix_fmt",
                        "yuv420p",
                        "-preset",
                        "ultrafast",
                        "-crf",
                        "21",
                        "-c:a",
                        "aac",
                        "-ac",
                        "2",
                        "-force_key_frames",
                        "expr:gte(t,n_forced*2)",
                        "-f",
                        "hls",
                        "-hls_time",
                        "2",
                        "-hls_list_size",
                        "8",
                        "-hls_flags",
                        "delete_segments+independent_segments+temp_file",
                        "-hls_segment_filename",
                        "segment-%05d.ts",
                        "live.m3u8",
                    ])
                    .current_dir(&directory)
                    .stderr(ffmpeg_log)
                    .stdin(Stdio::null())
                    .quiet()
                    .spawn()
                    .map_err(err)?,
            );
            Ok(Self {
                encoder,
                server,
                directory: directory.clone(),
                url: format!("http://{address}:{port}/live.m3u8"),
            })
        };
        let result = start();
        if result.is_err() {
            let _ = fs::remove_dir_all(&directory);
        }
        result
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        if self.server.0.try_wait().map_err(err)?.is_some() {
            return Err(fs::read_to_string(self.directory.join("server.log"))
                .unwrap_or_else(|_| tr("Servidor HLS encerrado.").into()));
        }
        if self
            .encoder
            .0
            .try_wait()
            .map_err(err)?
            .is_some_and(|x| !x.success())
        {
            return Err(fs::read_to_string(self.directory.join("ffmpeg.log"))
                .unwrap_or_else(|_| tr("Falha no encoder.").into()));
        }
        Ok(self.directory.join("live.m3u8").is_file())
    }
}

impl Drop for Broadcast {
    fn drop(&mut self) {
        for process in [&mut self.encoder.0, &mut self.server.0] {
            let _ = process.kill();
            let _ = process.wait();
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}
