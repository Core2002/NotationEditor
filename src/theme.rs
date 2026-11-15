//! Shared palette, generated textures and small UI helpers.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game::{Judgment, Rank};

pub const BG: Color = Color::srgb(0.010, 0.013, 0.026);
pub const PANEL: Color = Color::srgba(0.035, 0.045, 0.075, 0.92);
pub const PANEL_SOFT: Color = Color::srgba(0.045, 0.058, 0.095, 0.75);
pub const PANEL_HOVER: Color = Color::srgba(0.10, 0.14, 0.22, 0.96);
pub const ACCENT: Color = Color::srgb(0.35, 0.92, 1.0);
pub const ACCENT_DIM: Color = Color::srgb(0.16, 0.45, 0.55);
pub const TEXT: Color = Color::srgb(0.90, 0.94, 1.0);
pub const MUTED: Color = Color::srgb(0.48, 0.55, 0.68);
pub const FAINT: Color = Color::srgb(0.24, 0.29, 0.40);
pub const SUCCESS: Color = Color::srgb(0.42, 1.0, 0.66);
pub const WARNING: Color = Color::srgb(1.0, 0.72, 0.30);
pub const DANGER: Color = Color::srgb(1.0, 0.30, 0.42);

pub const LANE_COLORS: [Color; 6] = [
    Color::srgb(1.0, 0.30, 0.43),
    Color::srgb(1.0, 0.62, 0.25),
    Color::srgb(1.0, 0.89, 0.36),
    Color::srgb(0.30, 1.0, 0.72),
    Color::srgb(0.30, 0.79, 1.0),
    Color::srgb(0.70, 0.53, 1.0),
];

pub fn lane_color(lane: u8) -> Color {
    LANE_COLORS[(lane as usize) % LANE_COLORS.len()]
}

pub fn judgment_color(judgment: Judgment) -> Color {
    match judgment {
        Judgment::Perfect => Color::srgb(0.55, 1.0, 0.98),
        Judgment::Great => Color::srgb(0.42, 1.0, 0.60),
        Judgment::Good => Color::srgb(1.0, 0.82, 0.36),
        Judgment::Miss => DANGER,
    }
}

pub fn rank_color(rank: Rank) -> Color {
    match rank {
        Rank::S => Color::srgb(1.0, 0.85, 0.35),
        Rank::A => Color::srgb(0.40, 1.0, 0.82),
        Rank::B => Color::srgb(0.40, 0.78, 1.0),
        Rank::C => Color::srgb(1.0, 0.65, 0.35),
        Rank::D => MUTED,
    }
}

/// Textures generated once at startup. Avoids shipping bitmap art.
#[derive(Resource, Clone)]
pub struct UiAssets {
    pub glow: Handle<Image>,
    pub white: Handle<Image>,
}

#[derive(Component, Clone, Copy)]
pub struct ButtonVisual {
    pub normal: Color,
    pub hover: Color,
    pub pressed: Color,
}

impl Default for ButtonVisual {
    fn default() -> Self {
        Self {
            normal: PANEL,
            hover: PANEL_HOVER,
            pressed: ACCENT_DIM,
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn button_visuals(
    mut buttons: Query<
        (&Interaction, &ButtonVisual, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, visual, mut background) in &mut buttons {
        background.0 = match interaction {
            Interaction::Pressed => visual.pressed,
            Interaction::Hovered => visual.hover,
            Interaction::None => visual.normal,
        };
    }
}

pub fn text_font(size: f32) -> TextFont {
    TextFont::from_font_size(size)
}

pub fn build_textures(images: &mut Assets<Image>) -> UiAssets {
    UiAssets {
        glow: images.add(make_hex_glow_texture(128)),
        white: images.add(make_white_texture()),
    }
}

fn make_white_texture() -> Image {
    let size = 4u32;
    let data = vec![255u8; (size * size * 4) as usize];
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// A white soft-edged hexagon. Tinting happens through `Sprite::color`.
fn make_hex_glow_texture(size: u32) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let c = 30.0_f32.to_radians().cos();
    let s = 30.0_f32.to_radians().sin();
    for y in 0..size {
        for x in 0..size {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / size as f32 * 2.0 - Vec2::ONE;
            // Hexagonal distance (inradius-normalised).
            let d = p
                .dot(Vec2::new(c, s))
                .abs()
                .max(p.dot(Vec2::Y).abs())
                .max(p.dot(Vec2::new(-c, s)).abs());
            let core = smoothstep(0.79, 0.66, d);
            let glow = (1.0 - (d / 1.12)).clamp(0.0, 1.0).powf(2.1);
            let alpha = (core * 0.95 + glow * 0.50).clamp(0.0, 1.0);
            let shine = (1.0 - d).clamp(0.0, 1.0) * 0.15;
            data.extend_from_slice(&[
                255,
                255,
                255,
                ((alpha + shine).clamp(0.0, 1.0) * 255.0) as u8,
            ]);
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Deterministic star field for menu/backgrounds.
pub fn star_field(count: usize) -> Vec<(Vec2, f32, f32)> {
    let mut seed = 0x9E37_79B9u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut stars = Vec::with_capacity(count);
    for _ in 0..count {
        let x = (next() as f32 / u32::MAX as f32 - 0.5) * 1280.0;
        let y = (next() as f32 / u32::MAX as f32 - 0.5) * 800.0;
        let size = 0.7 + (next() as f32 / u32::MAX as f32) * 1.8;
        let phase = next() as f32 / u32::MAX as f32 * std::f32::consts::TAU;
        stars.push((Vec2::new(x, y), size, phase));
    }
    stars
}
