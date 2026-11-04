use crate::i18n::tr;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::{io::AsRawHandle, process::CommandExt};
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::{
    os::{fd::OwnedFd, unix::net::UnixStream},
    process::Stdio,
};

pub trait QuietCommand {
    fn quiet(&mut self) -> &mut Self;
}

impl QuietCommand for Command {
    fn quiet(&mut self) -> &mut Self {
        #[cfg(windows)]
        self.creation_flags(0x08000000);
        self
    }
}

fn tool(name: &str) -> PathBuf {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| name.into())
}

#[cfg(unix)]
fn media_tool(name: &str) -> PathBuf {
    env::var_os("ANI_MEDIA_DIR")
        .map(PathBuf::from)
        .map(|root| root.join("bin").join(name))
        .unwrap_or_else(|| tool(name))
}

#[derive(Clone, Copy)]
pub enum Output {
    #[cfg(test)]
    Headless,
    #[cfg(windows)]
    Embedded(isize),
    #[cfg(unix)]
    Window,
}
const PROPERTIES: &[&str] = &[
    "path",
    "media-title",
    "time-pos",
    "duration",
    "pause",
    "mute",
    "volume",
    "container-fps",
    "estimated-vf-fps",
    "display-fps",
    "width",
    "height",
    "video-codec",
    "audio-codec-name",
    "playlist",
    "playlist-pos",
    "track-list",
    "chapter-list",
    "chapter",
    "speed",
    "idle-active",
    "glsl-shaders",
    "panscan",
];
pub type State = BTreeMap<String, Value>;

#[derive(Clone)]
pub struct Paths {
    pub mpv: PathBuf,
    pub ffmpeg: PathBuf,
    pub config: PathBuf,
    pub app: PathBuf,
}

impl Paths {
    pub fn find() -> Self {
        let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("ani-player.exe"));
        let app = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
        #[cfg(windows)]
        let root = app
            .ancestors()
            .find(|p| p.join("mpv64/mpv.exe").is_file())
            .unwrap_or(&app);
        Self {
            mpv: env::var_os("ANI_MPV")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    #[cfg(windows)]
                    {
                        let adjacent = app.join("mpv.exe");
                        if adjacent.is_file() {
                            adjacent
                        } else if root.join("mpv64/mpv.exe").is_file() {
                            root.join("mpv64/mpv.exe")
                        } else {
                            tool("mpv.exe")
                        }
                    }
                    #[cfg(unix)]
                    media_tool("mpv")
                }),
            ffmpeg: env::var_os("ANI_FFMPEG")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    #[cfg(windows)]
                    {
                        let bundled = root.join("utils/ffmpeg.exe");
                        if bundled.is_file() {
                            bundled
                        } else {
                            tool("ffmpeg.exe")
                        }
                    }
                    #[cfg(unix)]
                    media_tool("ffmpeg")
                }),
            config: crate::i18n::config_dir(),
            app,
        }
    }

    pub fn python(&self) -> PathBuf {
        env::var_os("ANI_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                #[cfg(windows)]
                {
                    let bundled = self.mpv.with_file_name("python.exe");
                    if bundled.is_file() {
                        bundled
                    } else {
                        tool("python.exe")
                    }
                }
                #[cfg(unix)]
                media_tool("python3")
            })
    }
}

pub enum Update {
    Ready,
    Snapshot(State),
    Error(String),
}

enum Request {
    Command(Vec<Value>),
    Motion(Option<String>),
    Upscale(crate::upscale::Mode),
    Quit,
}

pub struct Engine {
    tx: Sender<Request>,
    pub updates: Receiver<Update>,
}

impl Engine {
    pub fn start(paths: Paths, output: Output) -> Self {
        let (tx, requests) = mpsc::channel();
        let (events, updates) = mpsc::channel();
        thread::spawn(move || {
            if let Err(error) = run(paths, output, requests, &events) {
                let _ = events.send(Update::Error(error));
            }
        });
        Self { tx, updates }
    }

    pub fn command(&self, args: impl IntoIterator<Item = Value>) {
        let _ = self.tx.send(Request::Command(args.into_iter().collect()));
    }

    pub fn words(&self, args: &[&str]) {
        self.command(args.iter().map(|s| json!(s)));
    }

    pub fn upscale(&self, mode: crate::upscale::Mode) {
        let _ = self.tx.send(Request::Upscale(mode));
    }

    pub fn motion(&self, filter: Option<String>) {
        let _ = self.tx.send(Request::Motion(filter));
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Quit);
    }
}

#[cfg(windows)]
pub(super) struct Job(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Job {
    pub(super) fn for_child(child: &Child) -> Result<Self, String> {
        use windows_sys::Win32::System::JobObjects::*;
        // Kill mpv and its descendants if the UI exits while IPC is busy.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(err(std::io::Error::last_os_error()));
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
            {
                return Err(err(std::io::Error::last_os_error()));
            }
            Ok(job)
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(windows)]
type Ipc = fs::File;
#[cfg(unix)]
type Ipc = UnixStream;

struct Pipe {
    writer: Ipc,
    reader: BufReader<Ipc>,
    id: u64,
    events: Sender<Update>,
}

impl Pipe {
    fn call(&mut self, args: &[Value]) -> Result<Value, String> {
        self.id += 1;
        writeln!(
            self.writer,
            "{}",
            json!({"command":args,"request_id":self.id})
        )
        .map_err(err)?;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).map_err(err)? == 0 {
                return Err(tr("O mpv encerrou a conexão.").into());
            }
            let row: Value = serde_json::from_str(&line).map_err(err)?;
            if row["event"] == "end-file" && row["reason"] == "error" {
                let message = row["file_error"]
                    .as_str()
                    .unwrap_or(tr("Falha ao abrir a mídia."));
                let _ = self.events.send(Update::Error(message.into()));
            }
            if row["request_id"] == self.id {
                return if row["error"] == "success" {
                    Ok(row["data"].clone())
                } else {
                    Err(row["error"].as_str().unwrap_or("Erro do mpv").into())
                };
            }
        }
    }
}

fn run(
    paths: Paths,
    output: Output,
    requests: Receiver<Request>,
    events: &Sender<Update>,
) -> Result<(), String> {
    if !paths.mpv.is_file() {
        return Err(
            tr("mpv não encontrado em {path}").replace("{path}", &paths.mpv.display().to_string())
        );
    }
    fs::create_dir_all(&paths.config).map_err(err)?;
    #[cfg(windows)]
    let pipe_path = format!(
        r"\\.\pipe\ani-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    #[cfg(unix)]
    let (file, peer) = UnixStream::pair().map_err(err)?;
    #[cfg(windows)]
    let engine_dir = paths
        .mpv
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    #[cfg(windows)]
    let tube_dir = Some(engine_dir.join("../extensions/tube"));
    #[cfg(unix)]
    let tube_dir = env::var_os("ANI_MEDIA_DIR").map(|root| PathBuf::from(root).join("bin"));
    let search_path = env::join_paths(
        [paths.app.clone()]
            .into_iter()
            .chain(tube_dir)
            .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )
    .map_err(err)?;
    let mut command = Command::new(&paths.mpv);
    command
        .args([
            "--idle=yes",
            "--terminal=no",
            "--osc=no",
            "--input-default-bindings=no",
            "--input-vo-keyboard=no",
            "--hwdec=auto-copy",
            "--panscan=1",
            "--ytdl=yes",
        ])
        .arg(format!("--config-dir={}", paths.config.display()))
        .env("PATH", search_path)
        .quiet();
    #[cfg(windows)]
    command
        .arg(format!("--input-ipc-server={pipe_path}"))
        .current_dir(engine_dir);
    #[cfg(windows)]
    {
        let script = engine_dir.join("Lib/site-packages/vapoursynth/vsscript.dll");
        if script.is_file() && env::var_os("VSSCRIPT_PATH").is_none() {
            command.env("VSSCRIPT_PATH", script);
        }
    }
    #[cfg(unix)]
    command
        .arg("--input-ipc-client=fd://0")
        .stdin(Stdio::from(OwnedFd::from(peer)));
    match output {
        #[cfg(windows)]
        Output::Embedded(window) => {
            command.arg(format!("--wid={window}")).arg("--vo=gpu-next");
        }
        #[cfg(test)]
        Output::Headless => {
            command.args(["--vo=null", "--ao=null"]);
        }
        #[cfg(unix)]
        Output::Window => {
            command.args([
                "--osc=yes",
                "--input-default-bindings=yes",
                "--input-vo-keyboard=yes",
                "--vo=gpu-next,gpu",
                "--keep-open=yes",
            ]);
        }
    }
    let mut child = Process(command.spawn().map_err(err)?);
    drop(command);
    #[cfg(windows)]
    let _job = Job::for_child(&child.0)?;
    #[cfg(windows)]
    let file = {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            if let Ok(file) = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&pipe_path)
            {
                break file;
            }
            if Instant::now() > deadline || child.0.try_wait().map_err(err)?.is_some() {
                return Err(tr("O mpv não iniciou a conexão.").into());
            }
            thread::sleep(Duration::from_millis(35));
        }
    };
    #[cfg(unix)]
    file.set_read_timeout(Some(Duration::from_secs(6)))
        .map_err(err)?;
    let mut pipe = Pipe {
        reader: BufReader::new(file.try_clone().map_err(err)?),
        writer: file,
        id: 0,
        events: events.clone(),
    };
    #[cfg(unix)]
    pipe.call(&[json!("get_property"), json!("idle-active")])?;
    let _ = events.send(Update::Ready);
    let mut filter_active = false;
    let mut next_poll = Instant::now();
    loop {
        if Instant::now() >= next_poll {
            if child.0.try_wait().map_err(err)?.is_some() {
                return Err("O motor mpv foi encerrado.".into());
            }
            let mut snapshot = State::new();
            for property in PROPERTIES {
                if let Ok(value) = pipe.call(&[json!("get_property"), json!(property)]) {
                    snapshot.insert((*property).into(), value);
                }
            }
            if events.send(Update::Snapshot(snapshot)).is_err() {
                return Ok(());
            }
            next_poll = Instant::now() + Duration::from_millis(230);
        }
        let result =
            match requests.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
                Ok(Request::Command(args)) => pipe.call(&args).map(|_| ()),
                Ok(Request::Motion(filter)) => (|| {
                    if filter_active {
                        pipe.call(&[json!("vf"), json!("remove"), json!("@ani-motion")])?;
                        filter_active = false;
                    }
                    if let Some(filter) = filter {
                        pipe.call(&[
                            json!("vf"),
                            json!("add"),
                            json!(format!("@ani-motion:{filter}")),
                        ])?;
                        filter_active = true;
                    }
                    Ok(())
                })(),
                Ok(Request::Upscale(mode)) => (|| {
                    let current = pipe.call(&[json!("get_property"), json!("glsl-shaders")])?;
                    let shaders = mode.apply(&paths.config, &current)?;
                    pipe.call(&[json!("set_property"), json!("glsl-shaders"), json!(shaders)])?;
                    Ok(())
                })(),
                Ok(Request::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            };
        if let Err(message) = result {
            let _ = events.send(Update::Error(message));
        }
    }
}

pub fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
