use crate::{engine::QuietCommand, i18n::tr, theme};
use eframe::egui;
use semver::Version;
use serde::Deserialize;
use std::{
    process::Command,
    sync::mpsc::{self, Receiver},
};

const REPO: &str = "https://github.com/Apolinario-coder/aniplayer";
const API: &str = "https://api.github.com/repos/Apolinario-coder/aniplayer/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

fn newer_release(json: &[u8], current: &Version) -> Option<Release> {
    let release: Release = serde_json::from_slice(json).ok()?;
    let version = Version::parse(release.tag_name.trim_start_matches('v')).ok()?;
    (!release.draft
        && !release.prerelease
        && version.pre.is_empty()
        && version.cmp_precedence(current).is_gt())
    .then_some(release)
}

pub struct Updates {
    pending: Receiver<Option<Release>>,
    release: Option<Release>,
}

impl Updates {
    pub fn start() -> Self {
        let (sender, pending) = mpsc::channel();
        std::thread::spawn(move || {
            let output = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
                .args([
                    "-q",
                    "--fail",
                    "--silent",
                    "--max-time",
                    "8",
                    "--max-filesize",
                    "1048576",
                    "--proto",
                    "=https",
                    "-H",
                    "Accept: application/vnd.github+json",
                    "-A",
                    concat!("Ani-Player/", env!("CARGO_PKG_VERSION")),
                    API,
                ])
                .quiet()
                .output()
                .ok();
            let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("valid package version");
            let release = output
                .filter(|out| out.status.success())
                .and_then(|out| newer_release(&out.stdout, &current));
            let _ = sender.send(release);
        });
        Self {
            pending,
            release: None,
        }
    }

    pub fn poll(&mut self) {
        if let Ok(release) = self.pending.try_recv() {
            self.release = release;
        }
    }

    pub fn visible(&self) -> bool {
        self.release.is_some()
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let Some(release) = &self.release else { return };
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("ani-player-update")).show(ctx, |ui| {
            ui.heading(tr("Atualização disponível"));
            ui.label(
                tr("A versão {version} está disponível. Deseja atualizar?")
                    .replace("{version}", release.tag_name.trim_start_matches('v')),
            );
            ui.horizontal(|ui| {
                if ui.add(theme::primary(tr("Baixar atualização"))).clicked() {
                    let url = format!("{REPO}/releases/tag/{}", release.tag_name);
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                    close = true;
                }
                close |= ui.button(tr("Mais tarde")).clicked();
            });
        });
        if close || response.should_close() {
            self.release = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_offers_newer_stable_releases() {
        let current = Version::new(1, 0, 0);
        for (tag, draft, prerelease, expected) in [
            ("v1.0.1", false, false, true),
            ("v1.0.0", false, false, false),
            ("v0.9.0", false, false, false),
            ("v2.0.0-beta.1", false, true, false),
            ("v2.0.0", true, false, false),
            ("../other", false, false, false),
        ] {
            let json = serde_json::json!({"tag_name":tag,"draft":draft,"prerelease":prerelease});
            assert_eq!(
                newer_release(json.to_string().as_bytes(), &current).is_some(),
                expected
            );
        }
    }
}
