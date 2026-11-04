use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/ani-player.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let script = out.join("ani-player.rc");
    fs::write(&script, "1 ICON \"assets/ani-player.ico\"\n").expect("write icon resource");
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let resource = out.join(if msvc {
        "ani-player.res"
    } else {
        "ani-player.o"
    });
    let status = if msvc {
        let rc = env::var_os("RC").map(PathBuf::from).or_else(|| {
            let sdk = PathBuf::from(env::var_os("ProgramFiles(x86)")?).join("Windows Kits/10/bin");
            fs::read_dir(sdk)
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("x64/rc.exe"))
                .filter(|path| path.is_file())
                .max()
        });
        Command::new(rc.unwrap_or_else(|| "rc.exe".into()))
            .arg("/nologo")
            .arg("/fo")
            .arg(&resource)
            .arg(&script)
            .status()
    } else {
        let windres = env::var_os("WINDRES").unwrap_or_else(|| "windres".into());
        Command::new(windres)
            .args(["-O", "coff"])
            .arg(&script)
            .arg(&resource)
            .status()
    };
    assert!(
        status.expect("run Windows resource compiler").success(),
        "compile Ani Player icon resource"
    );
    println!("cargo:rustc-link-arg-bin=ani-player={}", resource.display());
    if env::var_os("CARGO_FEATURE_WINDOWS_SETUP").is_some() {
        println!("cargo:rustc-link-arg-bin=ani-setup={}", resource.display());
    }
}
