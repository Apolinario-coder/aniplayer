use std::{
    env,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, ExitCode},
};

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let exe = env::current_exe()?;
    let root = exe
        .ancestors()
        .find(|p| p.join("extensions/tube/yt_dlp").is_dir())
        .ok_or("Módulo yt-dlp local indisponível.")?;
    let python = env::var_os("ANI_MPV")
        .map(PathBuf::from)
        .and_then(|mpv| mpv.parent().map(|p| p.join("python.exe")))
        .unwrap_or_else(|| root.join("mpv64/python.exe"));
    let status = Command::new(python)
        .args([
            "-X",
            "utf8",
            "-c",
            "import sys; sys.path.insert(0, sys.argv.pop(1)); import yt_dlp; yt_dlp.main()",
        ])
        .arg(root.join("extensions/tube"))
        .args(env::args_os().skip(1))
        .creation_flags(0x08000000)
        .status()?;
    Ok(ExitCode::from(
        status.code().unwrap_or(1).clamp(0, 255) as u8
    ))
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
