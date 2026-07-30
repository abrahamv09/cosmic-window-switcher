// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use cosmic_window_switcher::{
    CardSize, GridLayout, GridRect, SwitcherGrid, SwitcherItem, ThumbnailFrame, WindowId,
};

use super::icons::{IconImage, IconResolver};

const ICON_SIZE: u32 = 36;
const THUMBNAIL_PADDING: u32 = 12;
const THUMBNAIL_HEIGHT: u32 = 172;

pub(super) struct RenderedOverlay {
    pub(super) dimensions: OverlayDimensions,
    pub(super) pixels: Vec<u8>,
    pub(super) visible_windows: Vec<WindowId>,
    pub(super) layout: GridLayout,
}

#[derive(Clone, Copy)]
pub(super) struct OverlayDimensions {
    pub(super) logical_width: u32,
    pub(super) logical_height: u32,
    pub(super) scale: i32,
}

pub(super) struct OverlayRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    icons: IconResolver,
}

impl OverlayRenderer {
    pub(super) fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            icons: IconResolver::default(),
        }
    }

    pub(super) fn render(
        &mut self,
        grid: &mut SwitcherGrid,
        surface_logical_width: u32,
        surface_logical_height: u32,
        scale: i32,
    ) -> Result<RenderedOverlay> {
        let scale = scale.max(1);
        let physical_scale = u32::try_from(scale).context("invalid output scale")?;
        let font_scale = f32::from(u16::try_from(scale).context("output scale is too large")?);
        let layout = grid
            .layout(
                surface_logical_width.saturating_mul(4) / 5,
                surface_logical_height.saturating_mul(4) / 5,
                CardSize::Medium,
            )
            .centered_in(surface_logical_width, surface_logical_height);
        let visible_item_range = layout.visible_item_range();
        let logical_width = surface_logical_width;
        let logical_height = surface_logical_height;
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
        let visible_windows = grid.items()[visible_item_range.clone()]
            .iter()
            .map(|item| item.window().clone())
            .collect();

        fill_rect(
            &mut pixels,
            physical_width,
            Rect::new(0, 0, physical_width, physical_height),
            Color::rgba(24, 27, 36, 246),
        );

        for (index, item) in grid.items().iter().enumerate() {
            if !visible_item_range.contains(&index) {
                continue;
            }
            let bounds = layout
                .item_bounds(index)
                .context("visible Switcher Item has no layout bounds")?;
            self.draw_item(
                &mut pixels,
                physical_width,
                item,
                bounds,
                (physical_scale, font_scale),
            );
        }

        let mut bytes = Vec::with_capacity(pixels.len() * size_of::<u32>());
        for pixel in pixels {
            bytes.extend_from_slice(&pixel.to_ne_bytes());
        }
        Ok(RenderedOverlay {
            dimensions: OverlayDimensions {
                logical_width,
                logical_height,
                scale,
            },
            pixels: bytes,
            visible_windows,
            layout,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_item(
        &mut self,
        pixels: &mut [u32],
        surface_width: u32,
        item: &SwitcherItem,
        bounds: GridRect,
        scale: (u32, f32),
    ) {
        let (physical_scale, font_scale) = scale;
        let (item_width, item_height) = bounds.size();
        let x = bounds.x() * physical_scale;
        let y = bounds.y() * physical_scale;
        let item_rect = Rect::new(
            x,
            y,
            item_width * physical_scale,
            item_height * physical_scale,
        );
        let item_color = if item.is_selected() {
            Color::rgb(38, 92, 150)
        } else {
            Color::rgb(45, 49, 62)
        };
        fill_rect(pixels, surface_width, item_rect, item_color);
        if item.is_selected() {
            stroke_rect(
                pixels,
                surface_width,
                item_rect,
                3 * physical_scale,
                Color::rgb(124, 189, 255),
            );
        }

        let thumbnail_rect = Rect::new(
            x + THUMBNAIL_PADDING * physical_scale,
            y + THUMBNAIL_PADDING * physical_scale,
            (item_width - 2 * THUMBNAIL_PADDING) * physical_scale,
            THUMBNAIL_HEIGHT * physical_scale,
        );
        fill_rect(
            pixels,
            surface_width,
            thumbnail_rect,
            Color::rgb(14, 16, 22),
        );
        if let Some(thumbnail) = item.thumbnail() {
            draw_thumbnail(pixels, surface_width, thumbnail_rect, thumbnail);
        }

        let icon_rect = Rect::new(
            x + 16 * physical_scale,
            y + 190 * physical_scale,
            ICON_SIZE * physical_scale,
            ICON_SIZE * physical_scale,
        );
        let icon_drawn = self
            .icons
            .resolve(item.application_icon(), ICON_SIZE * physical_scale)
            .is_some_and(|icon| {
                draw_icon(pixels, surface_width, icon_rect, icon);
                true
            });
        if !icon_drawn {
            fill_rect(
                pixels,
                surface_width,
                icon_rect,
                application_color(item.application_id()),
            );
            self.draw_text(
                pixels,
                surface_width,
                &item.application_icon().fallback_monogram().to_string(),
                icon_rect.x + 10 * physical_scale,
                icon_rect.y + 5 * physical_scale,
                20 * physical_scale,
                24 * physical_scale,
                18.0 * font_scale,
                22.0 * font_scale,
                Color::rgb(255, 255, 255),
            );
        }
        self.draw_text(
            pixels,
            surface_width,
            item.title(),
            x + 64 * physical_scale,
            y + 197 * physical_scale,
            (item_width - 80) * physical_scale,
            28 * physical_scale,
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
        text: &str,
        x: u32,
        y: u32,
        text_width: u32,
        text_height: u32,
        font_size: f32,
        line_height: f32,
        color: Color,
    ) {
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        let available_width = u16::try_from(text_width).unwrap_or(u16::MAX);
        let available_height = u16::try_from(text_height).unwrap_or(u16::MAX);
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
                let Some(glyph_rect) = Rect::new(glyph_x, glyph_y, width, height)
                    .intersection(Rect::new(x, y, text_width, text_height))
                else {
                    return;
                };
                blend_rect(pixels, surface_width, glyph_rect, color);
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

    fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self
            .x
            .saturating_add(self.width)
            .min(other.x.saturating_add(other.width));
        let bottom = self
            .y
            .saturating_add(self.height)
            .min(other.y.saturating_add(other.height));
        (right > x && bottom > y).then(|| Self::new(x, y, right - x, bottom - y))
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
    if x >= surface_width {
        return None;
    }
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

fn draw_icon(pixels: &mut [u32], surface_width: u32, bounds: Rect, icon: &IconImage) {
    let offset_x = bounds.x + bounds.width.saturating_sub(icon.width()) / 2;
    let offset_y = bounds.y + bounds.height.saturating_sub(icon.height()) / 2;
    for icon_y in 0..icon.height() {
        for icon_x in 0..icon.width() {
            let Some(index) = icon_y
                .checked_mul(icon.width())
                .and_then(|index| index.checked_add(icon_x))
                .and_then(|index| index.checked_mul(4))
                .and_then(|index| usize::try_from(index).ok())
            else {
                continue;
            };
            let Some(rgba) = icon.pixels().get(index..index + 4) else {
                continue;
            };
            blend_rect(
                pixels,
                surface_width,
                Rect::new(offset_x + icon_x, offset_y + icon_y, 1, 1),
                Color::rgba(rgba[0], rgba[1], rgba[2], rgba[3]),
            );
        }
    }
}

fn draw_thumbnail(
    pixels: &mut [u32],
    surface_width: u32,
    bounds: Rect,
    thumbnail: &ThumbnailFrame,
) {
    let (width, height) = thumbnail.fitted_size(bounds.width, bounds.height);
    if width == 0 || height == 0 {
        return;
    }
    let offset_x = bounds.x + (bounds.width - width) / 2;
    let offset_y = bounds.y + (bounds.height - height) / 2;
    let (presentation_width, presentation_height) = thumbnail.presentation_size();
    for target_y in 0..height {
        let source_y = u64::from(target_y) * u64::from(presentation_height) / u64::from(height);
        for target_x in 0..width {
            let source_x = u64::from(target_x) * u64::from(presentation_width) / u64::from(width);
            let Some(color) = thumbnail
                .argb_pixel(
                    u32::try_from(source_x).unwrap_or(presentation_width - 1),
                    u32::try_from(source_y).unwrap_or(presentation_height - 1),
                )
                .map(Color)
            else {
                continue;
            };
            if let Some(target) = pixel_mut(
                pixels,
                surface_width,
                offset_x + target_x,
                offset_y + target_y,
            ) {
                *target = color.0;
            }
        }
    }
}
