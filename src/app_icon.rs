use eframe::egui::IconData;

pub fn load() -> IconData {
    let image = image::load_from_memory(include_bytes!("../assets/ani-player.png"))
        .expect("embedded Ani Player icon")
        .into_rgba8();
    let (width, height) = image.dimensions();
    IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}
