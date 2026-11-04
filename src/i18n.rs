use eframe::egui;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

const LANGUAGES: [(&str, &str); 7] = [
    ("en", "English"),
    ("pt-BR", "Português (Brasil)"),
    ("es", "Español"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("zh-Hans", "简体中文"),
    ("ja", "日本語"),
];
static LANGUAGE: AtomicUsize = AtomicUsize::new(0);
static STRINGS: OnceLock<HashMap<&'static str, [&'static str; 7]>> = OnceLock::new();

pub fn config_dir() -> PathBuf {
    std::env::var_os("ANI_PLAYER_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(windows)]
            {
                std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("Ani Player Anime")
            }
            #[cfg(target_os = "macos")]
            {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("Library/Application Support/Ani Player")
            }
            #[cfg(target_os = "linux")]
            {
                std::env::var_os("XDG_CONFIG_HOME")
                    .filter(|path| Path::new(path).is_absolute())
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
                    })
                    .unwrap_or_else(std::env::temp_dir)
                    .join("ani-player")
            }
        })
}

pub fn load(config: &Path) {
    let language = fs::read_to_string(config.join("language.txt"))
        .ok()
        .and_then(|code| LANGUAGES.iter().position(|(id, _)| *id == code.trim()))
        .unwrap_or(0);
    LANGUAGE.store(language, Ordering::Relaxed);
}

pub fn tr(text: &str) -> &str {
    let language = LANGUAGE.load(Ordering::Relaxed);
    STRINGS
        .get_or_init(|| {
            include_str!("../assets/i18n/messages.txt")
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    let columns: [&str; 7] = line
                        .split('|')
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap_or_else(|_| panic!("invalid translation row: {line}"));
                    (columns[1], columns)
                })
                .collect()
        })
        .get(text)
        .map_or(text, |columns| columns[language])
}

pub fn fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, data) in [
        ("ui-sc", &include_bytes!("../assets/i18n/ani-sc.otf")[..]),
        ("ui-ja", &include_bytes!("../assets/i18n/ani-jp.otf")[..]),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(data).into());
    }
    let root = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()))
        .join("Fonts");
    for (name, candidates) in [
        ("cjk-sc", &["msyh.ttc", "simsun.ttc", "simhei.ttf"][..]),
        ("cjk-ja", &["YuGothR.ttc", "meiryo.ttc", "msgothic.ttc"][..]),
    ] {
        if let Some(data) = candidates
            .iter()
            .find_map(|file| fs::read(root.join(file)).ok())
        {
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_owned(data).into());
        }
    }
    let japanese = LANGUAGE.load(Ordering::Relaxed) == 6;
    for fallback in if japanese {
        ["ui-ja", "cjk-ja", "ui-sc", "cjk-sc"]
    } else {
        ["ui-sc", "cjk-sc", "ui-ja", "cjk-ja"]
    } {
        if fonts.font_data.contains_key(fallback) {
            for names in fonts.families.values_mut() {
                names.push(fallback.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}

pub fn menu(ui: &mut egui::Ui, config: &Path) -> bool {
    let current = LANGUAGE.load(Ordering::Relaxed);
    let mut changed = false;
    ui.menu_button(tr("Idioma"), |ui| {
        for (index, (code, name)) in LANGUAGES.iter().enumerate() {
            if ui.selectable_label(index == current, *name).clicked() {
                if index != current {
                    LANGUAGE.store(index, Ordering::Relaxed);
                    fonts(ui.ctx());
                    changed = true;
                }
                let _ = fs::create_dir_all(config)
                    .and_then(|()| fs::write(config.join("language.txt"), code));
                ui.close_menu();
            }
        }
    });
    changed
}
