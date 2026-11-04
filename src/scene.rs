use crate::theme::{CORNER, PAGE, STAGE};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

pub struct Scene {
    texture: egui::TextureHandle,
    fade: egui::TextureHandle,
    size: Vec2,
}

impl Scene {
    pub fn load(ctx: &egui::Context) -> Option<Self> {
        let decoded = image::load_from_memory(include_bytes!("../assets/estacao.png"))
            .ok()?
            .into_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        Some(Self {
            texture: ctx.load_texture("estacao-ao-anoitecer", image, egui::TextureOptions::LINEAR),
            fade: ctx.load_texture(
                "estacao-fade",
                horizontal_fade(),
                egui::TextureOptions::LINEAR,
            ),
            size: Vec2::new(size[0] as f32, size[1] as f32),
        })
    }

    pub fn paint(&self, ui: &egui::Ui, rect: Rect, compact: bool) {
        let image_rect = if compact {
            Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right(), rect.top() + rect.height() * 0.45),
            )
        } else {
            rect
        };
        let uv = self.cover_uv(image_rect, if compact { 0.67 } else { 0.65 });
        let corners = if compact {
            egui::CornerRadius {
                nw: CORNER,
                ne: CORNER,
                sw: 0,
                se: 0,
            }
        } else {
            egui::CornerRadius::same(CORNER)
        };
        egui::Image::new((self.texture.id(), self.size))
            .uv(uv)
            .corner_radius(corners)
            .paint_at(ui, image_rect);
        paint_fade(ui, image_rect, corners, compact, self.fade.id());
    }

    pub fn fullscreen_background(&self, rect: Rect) -> egui::Shape {
        let uv = self.cover_uv(rect, 0.65);
        let fade_uv = Rect::from_min_max(Pos2::new(0.0, 0.5), Pos2::new(1.0, 0.5));
        let top = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 76.0));
        let bottom = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + rect.height() * 0.58),
            rect.right_bottom(),
        );
        egui::Shape::Vec(vec![
            egui::Shape::image(self.texture.id(), rect, uv, Color32::WHITE),
            egui::Shape::image(self.fade.id(), rect, fade_uv, Color32::WHITE),
            vertical_fade(
                top,
                Color32::from_rgba_unmultiplied(PAGE.r(), PAGE.g(), PAGE.b(), 200),
                Color32::TRANSPARENT,
            ),
            vertical_fade(bottom, Color32::TRANSPARENT, PAGE),
        ])
    }

    fn cover_uv(&self, rect: Rect, anchor: f32) -> Rect {
        let ratio = rect.aspect_ratio() / (self.size.x / self.size.y);
        if ratio < 1.0 {
            let start = (1.0 - ratio) * anchor;
            Rect::from_min_max(Pos2::new(start, 0.0), Pos2::new(start + ratio, 1.0))
        } else {
            let visible = 1.0 / ratio;
            let start = (1.0 - visible) * 0.55;
            Rect::from_min_max(Pos2::new(0.0, start), Pos2::new(1.0, start + visible))
        }
    }
}

fn vertical_fade(rect: Rect, top: Color32, bottom: Color32) -> egui::Shape {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 2, 1);
    mesh.add_triangle(1, 2, 3);
    egui::Shape::mesh(mesh)
}

fn horizontal_fade() -> egui::ColorImage {
    let stops = [(0.0, 246.0), (0.54, 232.0), (0.78, 70.0), (1.0, 8.0)];
    let pixels = (0..256)
        .map(|index| {
            let x = index as f32 / 255.0;
            let [(left, from), (right, to)] = stops
                .windows(2)
                .find(|pair| x <= pair[1].0)
                .expect("gradient ends at 1.0")
            else {
                unreachable!()
            };
            let alpha = (from + (to - from) * (x - left) / (right - left)).round() as u8;
            Color32::from_rgba_unmultiplied(PAGE.r(), PAGE.g(), PAGE.b(), alpha)
        })
        .collect();
    egui::ColorImage {
        size: [256, 1],
        pixels,
    }
}

fn paint_fade(
    ui: &egui::Ui,
    rect: Rect,
    corners: egui::CornerRadius,
    compact: bool,
    texture: egui::TextureId,
) {
    if compact {
        // Only the flat lower edge fades into the caption surface.
        let mut fade_rect = rect;
        fade_rect.min.y = (rect.bottom() - 25.0).max(rect.top());
        ui.painter()
            .add(vertical_fade(fade_rect, Color32::TRANSPARENT, STAGE));
        return;
    }

    // The gradient shares the image's rounded geometry and antialiased edge.
    let primitives = ui.ctx().tessellate(
        vec![egui::epaint::ClippedShape {
            clip_rect: ui.clip_rect(),
            shape: egui::Shape::rect_filled(rect, corners, Color32::WHITE),
        }],
        ui.ctx().pixels_per_point(),
    );
    for primitive in primitives {
        let egui::epaint::Primitive::Mesh(mut mesh) = primitive.primitive else {
            continue;
        };
        mesh.texture_id = texture;
        for vertex in &mut mesh.vertices {
            let x = ((vertex.pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            vertex.uv = Pos2::new(x, 0.5);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
    }
}
