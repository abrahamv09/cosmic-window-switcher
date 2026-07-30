// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use cosmic_window_switcher::{ApplicationIcon, SwitcherCard, SwitcherGrid};

const CARD_WIDTH: u32 = 220;
const CARD_HEIGHT: u32 = 116;
const CARD_GAP: u32 = 12;
const GRID_PADDING: u32 = 16;
const ICON_SIZE: u32 = 56;

pub(super) struct RenderedOverlay {
    pub(super) logical_width: u32,
    pub(super) logical_height: u32,
    pub(super) scale: i32,
    pub(super) pixels: Vec<u8>,
}

pub(super) struct OverlayRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl OverlayRenderer {
    pub(super) fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub(super) fn render(
        &mut self,
        grid: &SwitcherGrid,
        maximum_logical_width: u32,
        scale: i32,
    ) -> Result<RenderedOverlay> {
        let scale = scale.max(1);
        let physical_scale = u32::try_from(scale).context("invalid output scale")?;
        let font_scale = f32::from(u16::try_from(scale).context("output scale is too large")?);
        let card_count = u32::try_from(grid.cards().len()).context("too many Switcher Items")?;
        let available_width = maximum_logical_width.max(CARD_WIDTH + 2 * GRID_PADDING);
        let maximum_columns =
            ((available_width - 2 * GRID_PADDING + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).max(1);
        let columns = card_count.clamp(1, maximum_columns);
        let rows = card_count.div_ceil(columns);
        let logical_width =
            2 * GRID_PADDING + columns * CARD_WIDTH + columns.saturating_sub(1) * CARD_GAP;
        let logical_height =
            2 * GRID_PADDING + rows * CARD_HEIGHT + rows.saturating_sub(1) * CARD_GAP;
        let physical_width = logical_width
            .checked_mul(physical_scale)
            .context("Switcher Grid width overflow")?;
        let physical_height = logical_height
            .checked_mul(physical_scale)
            .context("Switcher Grid height overflow")?;
        let pixel_count = physical_width
            .checked_mul(physical_height)
            .context("Switcher Grid area overflow")?;
        let mut pixels =
            vec![0_u32; usize::try_from(pixel_count).context("Switcher Grid is too large")?];

        fill_rect(
            &mut pixels,
            physical_width,
            Rect::new(0, 0, physical_width, physical_height),
            Color::rgba(24, 27, 36, 246),
        );

        for (index, card) in grid.cards().iter().enumerate() {
            self.draw_card(
                &mut pixels,
                (physical_width, physical_height),
                card,
                u32::try_from(index).context("too many Switcher Items")?,
                columns,
                (physical_scale, font_scale),
            );
        }

        let mut bytes = Vec::with_capacity(pixels.len() * size_of::<u32>());
        for pixel in pixels {
            bytes.extend_from_slice(&pixel.to_ne_bytes());
        }
        Ok(RenderedOverlay {
            logical_width,
            logical_height,
            scale,
            pixels: bytes,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_card(
        &mut self,
        pixels: &mut [u32],
        surface_size: (u32, u32),
        card: &SwitcherCard,
        index: u32,
        columns: u32,
        scale: (u32, f32),
    ) {
        let (surface_width, surface_height) = surface_size;
        let (physical_scale, font_scale) = scale;
        let column = index % columns;
        let row = index / columns;
        let x = (GRID_PADDING + column * (CARD_WIDTH + CARD_GAP)) * physical_scale;
        let y = (GRID_PADDING + row * (CARD_HEIGHT + CARD_GAP)) * physical_scale;
        let card_rect = Rect::new(
            x,
            y,
            CARD_WIDTH * physical_scale,
            CARD_HEIGHT * physical_scale,
        );
        let card_color = if card.is_selected() {
            Color::rgb(38, 92, 150)
        } else {
            Color::rgb(45, 49, 62)
        };
        fill_rect(pixels, surface_width, card_rect, card_color);
        if card.is_selected() {
            stroke_rect(
                pixels,
                surface_width,
                card_rect,
                3 * physical_scale,
                Color::rgb(124, 189, 255),
            );
        }

        let icon_rect = Rect::new(
            x + 16 * physical_scale,
            y + 18 * physical_scale,
            ICON_SIZE * physical_scale,
            ICON_SIZE * physical_scale,
        );
        fill_rect(
            pixels,
            surface_width,
            icon_rect,
            application_color(card.application_id()),
        );
        let ApplicationIcon::Monogram(monogram) = card.application_icon();
        self.draw_text(
            pixels,
            surface_width,
            surface_height,
            &monogram.to_string(),
            icon_rect.x + 18 * physical_scale,
            icon_rect.y + 11 * physical_scale,
            24.0 * font_scale,
            28.0 * font_scale,
            Color::rgb(255, 255, 255),
        );
        self.draw_text(
            pixels,
            surface_width,
            surface_height,
            card.title(),
            x + 84 * physical_scale,
            y + 26 * physical_scale,
            16.0 * font_scale,
            22.0 * font_scale,
            Color::rgb(250, 251, 255),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &mut self,
        pixels: &mut [u32],
        surface_width: u32,
        surface_height: u32,
        text: &str,
        x: u32,
        y: u32,
        font_size: f32,
        line_height: f32,
        color: Color,
    ) {
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        let available_width = u16::try_from(surface_width.saturating_sub(x)).unwrap_or(u16::MAX);
        let available_height = u16::try_from(surface_height.saturating_sub(y)).unwrap_or(u16::MAX);
        buffer.set_size(
            Some(f32::from(available_width)),
            Some(f32::from(available_height)),
        );
        buffer.set_wrap(Wrap::None);
        buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        buffer.draw(
            &mut self.swash_cache,
            color,
            |glyph_x, glyph_y, width, height, color| {
                let Some(glyph_x) = i64::from(x).checked_add(i64::from(glyph_x)) else {
                    return;
                };
                let Some(glyph_y) = i64::from(y).checked_add(i64::from(glyph_y)) else {
                    return;
                };
                if glyph_x < 0 || glyph_y < 0 {
                    return;
                }
                let (Ok(glyph_x), Ok(glyph_y)) = (u32::try_from(glyph_x), u32::try_from(glyph_y))
                else {
                    return;
                };
                blend_rect(
                    pixels,
                    surface_width,
                    Rect::new(glyph_x, glyph_y, width, height),
                    color,
                );
            },
        );
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Rect {
    const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

fn fill_rect(pixels: &mut [u32], surface_width: u32, rect: Rect, color: Color) {
    let pixel = color.0;
    for y in rect.y..rect.y.saturating_add(rect.height) {
        for x in rect.x..rect.x.saturating_add(rect.width) {
            if let Some(target) = pixel_mut(pixels, surface_width, x, y) {
                *target = pixel;
            }
        }
    }
}

fn stroke_rect(pixels: &mut [u32], surface_width: u32, rect: Rect, thickness: u32, color: Color) {
    fill_rect(
        pixels,
        surface_width,
        Rect::new(rect.x, rect.y, rect.width, thickness),
        color,
    );
    fill_rect(
        pixels,
        surface_width,
        Rect::new(
            rect.x,
            rect.y.saturating_add(rect.height.saturating_sub(thickness)),
            rect.width,
            thickness,
        ),
        color,
    );
    fill_rect(
        pixels,
        surface_width,
        Rect::new(rect.x, rect.y, thickness, rect.height),
        color,
    );
    fill_rect(
        pixels,
        surface_width,
        Rect::new(
            rect.x.saturating_add(rect.width.saturating_sub(thickness)),
            rect.y,
            thickness,
            rect.height,
        ),
        color,
    );
}

fn blend_rect(pixels: &mut [u32], surface_width: u32, rect: Rect, source: Color) {
    let (source_red, source_green, source_blue, source_alpha) = source.as_rgba_tuple();
    for y in rect.y..rect.y.saturating_add(rect.height) {
        for x in rect.x..rect.x.saturating_add(rect.width) {
            let Some(target) = pixel_mut(pixels, surface_width, x, y) else {
                continue;
            };
            let target_color = Color(*target);
            let inverse_alpha = u16::from(u8::MAX - source_alpha);
            let blend = |source_channel: u8, target_channel: u8| {
                let value = u16::from(source_channel) * u16::from(source_alpha)
                    + u16::from(target_channel) * inverse_alpha;
                u8::try_from(value / u16::from(u8::MAX)).unwrap_or(u8::MAX)
            };
            *target = Color::rgba(
                blend(source_red, target_color.r()),
                blend(source_green, target_color.g()),
                blend(source_blue, target_color.b()),
                u8::MAX,
            )
            .0;
        }
    }
}

fn pixel_mut(pixels: &mut [u32], surface_width: u32, x: u32, y: u32) -> Option<&mut u32> {
    let index = y.checked_mul(surface_width)?.checked_add(x)?;
    pixels.get_mut(usize::try_from(index).ok()?)
}

fn application_color(application_id: &str) -> Color {
    let hash = application_id.bytes().fold(0x811C_9DC5_u32, |hash, byte| {
        hash.wrapping_mul(16_777_619) ^ u32::from(byte)
    });
    Color::rgb(
        72 + ((hash >> 16) & 0x7F) as u8,
        72 + ((hash >> 8) & 0x7F) as u8,
        72 + (hash & 0x7F) as u8,
    )
}
