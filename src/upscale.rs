use crate::i18n::tr;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off,
    A,
    B,
    C,
}

impl Mode {
    pub const ALL: [Self; 4] = [Self::Off, Self::A, Self::B, Self::C];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => tr("Desligado"),
            Self::A => tr("A · Para fontes 1080p"),
            Self::B => tr("B · Para fontes 720p"),
            Self::C => tr("C · Para fontes 480p"),
        }
    }

    pub fn current(config: &Path, shaders: &Value) -> Self {
        let Some(shaders) = shaders.as_array() else {
            return Self::Off;
        };
        Self::ALL
            .into_iter()
            .skip(1)
            .find(|mode| {
                let marker = match mode {
                    Self::A => 1,
                    Self::B => 2,
                    Self::C => 4,
                    Self::Off => unreachable!(),
                };
                let path = shader_path(config, marker);
                shaders
                    .iter()
                    .any(|shader| shader.as_str() == path.to_str())
            })
            .unwrap_or(Self::Off)
    }

    pub fn apply(self, config: &Path, existing: &Value) -> Result<Vec<String>, String> {
        let directory = config.join(SHADER_DIR);
        let mut shaders: Vec<String> = existing
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|shader| Path::new(shader).parent() != Some(directory.as_path()))
            .map(str::to_owned)
            .collect();

        // Order from Anime4K's Windows higher-end GPU template.
        let order: &[usize] = match self {
            Self::Off => &[],
            Self::A => &[0, 1, 3, 5, 6, 7],
            Self::B => &[0, 2, 3, 5, 6, 7],
            Self::C => &[0, 4, 5, 6, 7],
        };
        if !order.is_empty() {
            fs::create_dir_all(&directory).map_err(crate::engine::err)?;
        }
        for &index in order {
            let path = shader_path(config, index);
            let content = SHADERS[index].1;
            if fs::read(&path).ok().as_deref() != Some(content) {
                fs::write(&path, content).map_err(crate::engine::err)?;
            }
            shaders.push(
                path.to_str()
                    .ok_or(tr("Caminho dos shaders inválido."))?
                    .into(),
            );
        }
        Ok(shaders)
    }
}

const SHADER_DIR: &str = "anime4k-7684e95";

fn shader_path(config: &Path, index: usize) -> PathBuf {
    config
        .join(SHADER_DIR)
        .join(format!("Anime4K_{}.glsl", SHADERS[index].0))
}

macro_rules! shader {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../assets/anime4k/Anime4K_", $name, ".glsl")),
        )
    };
}

const SHADERS: [(&str, &[u8]); 8] = [
    shader!("Clamp_Highlights"),
    shader!("Restore_CNN_VL"),
    shader!("Restore_CNN_Soft_VL"),
    shader!("Upscale_CNN_x2_VL"),
    shader!("Upscale_Denoise_CNN_x2_VL"),
    shader!("AutoDownscalePre_x2"),
    shader!("AutoDownscalePre_x4"),
    shader!("Upscale_CNN_x2_M"),
];
