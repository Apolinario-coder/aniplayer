use crate::{
    engine::{Engine, Output, Paths, QuietCommand, State, Update},
    motion::{self, Broadcast, Export},
};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn expect_state(engine: &Engine, description: &str, check: impl Fn(&State) -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let event = engine
            .updates
            .recv_timeout(end.saturating_duration_since(Instant::now()));
        match event {
            Ok(Update::Snapshot(state)) if check(&state) => return,
            Ok(Update::Error(error)) => panic!("{description}: {error}"),
            Ok(_) => {}
            Err(error) => panic!("{description}: {error}"),
        }
    }
}

fn fps(state: &State, property: &str, target: f64) -> bool {
    state
        .get(property)
        .and_then(|x| x.as_f64())
        .is_some_and(|x| (x - target).abs() < 0.5)
}

fn http(port: u16, endpoint: &str) -> Vec<u8> {
    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    write!(client, "GET {endpoint} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    assert!(response.starts_with(b"HTTP/1.0 200"));
    let headers = response.windows(4).position(|x| x == b"\r\n\r\n").unwrap();
    response.split_off(headers + 4)
}

#[test]
#[ignore = "usa mpv, FFmpeg e RIFE da instalação de referência"]
fn playback_motion_export_and_stream() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let folder = Sandbox(std::env::temp_dir().join(format!("ani-rust-test-{stamp}")));
    fs::create_dir(&folder.0).unwrap();
    let mut paths = Paths::find();
    paths.config = folder.0.join("config");
    let clip = folder.0.join("Vídeo ç de teste.mp4");
    assert!(
        Command::new(&paths.ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=24",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440",
                "-t",
                "10",
                "-c:v",
                "libx264",
                "-c:a",
                "aac"
            ])
            .arg(&clip)
            .quiet()
            .status()
            .unwrap()
            .success()
    );
    let engine = Engine::start(paths.clone(), Output::Headless);
    engine.command([
        json!("loadfile"),
        json!(clip.to_str().unwrap()),
        json!("replace"),
    ]);
    expect_state(&engine, "reprodução a 24 fps", |s| {
        fps(s, "container-fps", 24.0)
    });
    for (mode, count) in [
        (crate::upscale::Mode::A, 6),
        (crate::upscale::Mode::B, 6),
        (crate::upscale::Mode::C, 5),
        (crate::upscale::Mode::Off, 0),
    ] {
        engine.upscale(mode);
        expect_state(&engine, "presets Anime4K via IPC", |s| {
            s.get("glsl-shaders").is_some_and(|shaders| {
                shaders.as_array().map_or(0, Vec::len) == count
                    && crate::upscale::Mode::current(&paths.config, shaders) == mode
            })
        });
    }
    expect_state(&engine, "preencher por padrão", |s| fps(s, "panscan", 1.0));
    engine.words(&["set", "panscan", "0"]);
    expect_state(&engine, "ajustar enquadramento", |s| fps(s, "panscan", 0.0));
    engine.words(&["set", "pause", "yes"]);
    expect_state(&engine, "pausa", |s| s["pause"] == true);
    engine.command([json!("seek"), json!(2.0), json!("absolute"), json!("exact")]);
    expect_state(&engine, "busca", |s| {
        s.get("time-pos")
            .and_then(|x| x.as_f64())
            .is_some_and(|p| (p - 2.0).abs() < 0.2)
    });
    engine.command([
        json!("loadfile"),
        json!(clip.to_str().unwrap()),
        json!("append-play"),
    ]);
    expect_state(&engine, "fila", |s| {
        s.get("playlist")
            .and_then(|v| v.as_array())
            .is_some_and(|v| v.len() == 2)
    });
    let subtitle = folder.0.join("teste.srt");
    fs::write(
        &subtitle,
        "1\n00:00:00,000 --> 00:00:05,000\nLegenda de teste\n",
    )
    .unwrap();
    engine.words(&["sub-add", subtitle.to_str().unwrap(), "select"]);
    expect_state(&engine, "legenda externa", |s| {
        s.get("track-list")
            .and_then(|v| v.as_array())
            .is_some_and(|list| {
                list.iter()
                    .any(|x| x["type"] == "sub" && x["selected"] == true)
            })
    });
    engine.words(&["set", "pause", "no"]);
    if cfg!(windows) || std::env::var_os("ANI_RIFE_DIR").is_some() {
        engine.motion(Some(motion::rife(&paths, Some(60.0)).unwrap()));
        expect_state(&engine, "RIFE a 60 fps", |s| {
            fps(s, "estimated-vf-fps", 60.0)
        });
        engine.motion(Some(motion::rife(&paths, None).unwrap()));
        expect_state(&engine, "RIFE a 48 fps", |s| {
            fps(s, "estimated-vf-fps", 48.0)
        });
    }

    let output = folder.0.join("Exportado.mkv");
    let mut export = Export::start(&paths, &clip, output.clone(), 60.0).unwrap();
    let deadline = Instant::now() + Duration::from_secs(40);
    while !export.poll().unwrap() {
        assert!(Instant::now() < deadline, "exportação demorou demais");
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(export);
    engine.motion(None);
    engine.command([
        json!("loadfile"),
        json!(output.to_str().unwrap()),
        json!("replace"),
    ]);
    expect_state(&engine, "exportação a 60 fps com áudio", |s| {
        fps(s, "container-fps", 60.0)
            && s.get("audio-codec-name").and_then(|v| v.as_str()) == Some("aac")
    });

    let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let mut stream = Broadcast::start(&paths, &clip, 60.0, Ipv4Addr::LOCALHOST, port).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !stream.poll().unwrap() {
        assert!(Instant::now() < deadline, "HLS demorou demais");
        std::thread::sleep(Duration::from_millis(100));
    }
    let manifest = String::from_utf8(http(port, "/live.m3u8")).unwrap();
    assert!(manifest.starts_with("#EXTM3U"));
    let segment = manifest.lines().find(|line| line.ends_with(".ts")).unwrap();
    assert_eq!(http(port, &format!("/{segment}"))[0], 0x47);
    engine.command([json!("loadfile"), json!(stream.url), json!("replace")]);
    expect_state(&engine, "reprodução HLS por URL", |s| {
        s.get("path").and_then(|v| v.as_str()) == Some(stream.url.as_str())
            && s.get("width").and_then(|v| v.as_i64()) == Some(320)
    });
    engine.words(&["stop"]);
    expect_state(&engine, "parada", |s| {
        s.get("idle-active").and_then(|v| v.as_bool()) == Some(true)
    });
}
