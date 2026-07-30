// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use cosmic_window_switcher::{
    CardSize, FractionalScale, GridLayout, GridRect, OverlayPresentation, SwitcherGrid,
    SwitcherItem, ThumbnailFrame,
};

use super::icons::{IconImage, IconResolver};

#[derive(Clone, Copy)]
struct CardGeometry {
    thumbnail_padding: u32,
    footer_height: u32,
    footer_padding: u32,
    icon_size: u32,
    title_gap: u32,
    title_font_size: f32,
    title_line_height: f32,
}

fn card_geometry(item_width: u32, item_height: u32) -> CardGeometry {
    let footer_height = (item_height / 6).max(1);
    let title_font_size = f32::from(u16::try_from(item_height).unwrap_or(u16::MAX)) / 22.0;
    CardGeometry {
        thumbnail_padding: (item_width / 64).max(3),
        footer_height,
        footer_padding: (item_width / 32).max(3),
        icon_size: footer_height.saturating_mul(2) / 3,
        title_gap: (item_width / 64).max(3),
        title_font_size,
        title_line_height: title_font_size * 1.25,
    }
}

pub(super) struct RenderedOverlay {
    pub(super) dimensions: OverlayDimensions,
    pub(super) pixels: Vec<u8>,
    pub(super) layout: GridLayout,
}

#[derive(Clone, Copy)]
pub(super) struct OverlayDimensions {
    pub(super) logical_width: u32,
    pub(super) logical_height: u32,
    physical_width: u32,
    physical_height: u32,
    pub(super) buffer_scale: i32,
}

impl OverlayDimensions {
    pub(super) const fn physical_size(self) -> (u32, u32) {
        (self.physical_width, self.physical_height)
    }
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
        card_size: CardSize,
        scale: FractionalScale,
        presentation: OverlayPresentation,
    ) -> Result<RenderedOverlay> {
        let font_scale = f32::from(
            u16::try_from(scale.protocol_units()).context("fractional scale is too large")?,
        ) / 120.0;
        let layout = grid
            .layout(
                surface_logical_width.saturating_mul(4) / 5,
                surface_logical_height.saturating_mul(4) / 5,
                card_size,
            )
            .centered_in(surface_logical_width, surface_logical_height);
        let visible_item_range = layout.visible_item_range();
        let logical_width = surface_logical_width;
        let logical_height = surface_logical_height;
        let (physical_width, physical_height) = scale.physical_size(logical_width, logical_height);
        let pixel_count = physical_width
            .checked_mul(physical_height)
            .context("Switcher Grid area overflow")?;
        let mut pixels =
            vec![0_u32; usize::try_from(pixel_count).context("Switcher Grid is too large")?];
        let background = Color::rgba(24, 27, 36, presentation.dimming().alpha());
        fill_rect(
            &mut pixels,
            physical_width,
            Rect::new(0, 0, physical_width, physical_height),
            background,
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
                (scale, font_scale),
                presentation,
            );
        }
        clip_items_to_viewport(
            &mut pixels,
            physical_width,
            physical_height,
            layout.viewport_bounds(),
            scale,
            background,
        );
        apply_opacity(&mut pixels, presentation.rendered_opacity());

        let mut bytes = Vec::with_capacity(pixels.len() * size_of::<u32>());
        for pixel in pixels {
            bytes.extend_from_slice(&pixel.to_ne_bytes());
        }
        Ok(RenderedOverlay {
            dimensions: OverlayDimensions {
                logical_width,
                logical_height,
                physical_width,
                physical_height,
                buffer_scale: scale.ceiling_integer(),
            },
            pixels: bytes,
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
        scale: (FractionalScale, f32),
        presentation: OverlayPresentation,
    ) {
        let (scale, font_scale) = scale;
        let (item_width, item_height) = bounds.size();
        let x = scale.physical_length(bounds.x());
        let y = scale.physical_length(bounds.y());
        let physical_item_width = scale.physical_length(item_width);
        let physical_item_height = scale.physical_length(item_height);
        let item_rect = Rect::new(x, y, physical_item_width, physical_item_height);
        let item_color = if presentation.high_contrast() {
            Color::rgb(0, 0, 0)
        } else if item.is_selected() {
            Color::rgb(38, 92, 150)
        } else {
            Color::rgb(45, 49, 62)
        };
        fill_rect(pixels, surface_width, item_rect, item_color);
        if item.is_selected() || presentation.high_contrast() {
            stroke_rect(
                pixels,
                surface_width,
                item_rect,
                scale.physical_length(if item.is_selected() { 4 } else { 1 }),
                if presentation.high_contrast() {
                    Color::rgb(255, 255, 255)
                } else {
                    Color::rgb(124, 189, 255)
                },
            );
        }

        let geometry = card_geometry(item_width, item_height);
        let thumbnail_padding = geometry.thumbnail_padding;
        let footer_height = geometry.footer_height;
        let thumbnail_rect = Rect::new(
            x + scale.physical_length(thumbnail_padding),
            y + scale.physical_length(thumbnail_padding),
            scale.physical_length(item_width.saturating_sub(2 * thumbnail_padding)),
            scale
                .physical_length(item_height.saturating_sub(footer_height + 2 * thumbnail_padding)),
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

        let footer_top = item_height.saturating_sub(footer_height);
        let footer_padding = geometry.footer_padding;
        let icon_size = geometry.icon_size;
        let icon_y = footer_top + footer_height.saturating_sub(icon_size) / 2;
        let icon_rect = Rect::new(
            x + scale.physical_length(footer_padding),
            y + scale.physical_length(icon_y),
            scale.physical_length(icon_size),
            scale.physical_length(icon_size),
        );
        let icon_drawn = self
            .icons
            .resolve(item.application_icon(), scale.physical_length(icon_size))
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
                icon_rect.x + icon_rect.width / 4,
                icon_rect.y + icon_rect.height / 8,
                icon_rect.width / 2,
                icon_rect.height.saturating_mul(3) / 4,
                (f32::from(u16::try_from(icon_size).unwrap_or(u16::MAX)) * 0.5) * font_scale,
                (f32::from(u16::try_from(icon_size).unwrap_or(u16::MAX)) * 0.625) * font_scale,
                Color::rgb(255, 255, 255),
            );
        }
        let title_x = footer_padding + icon_size + geometry.title_gap;
        let title_y = footer_top + footer_height.saturating_sub(icon_size) / 2;
        self.draw_text(
            pixels,
            surface_width,
            item.title(),
            x + scale.physical_length(title_x),
            y + scale.physical_length(title_y),
            scale.physical_length(item_width.saturating_sub(title_x + footer_padding)),
            scale.physical_length(icon_size),
            geometry.title_font_size * font_scale,
            geometry.title_line_height * font_scale,
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

fn clip_items_to_viewport(
    pixels: &mut [u32],
    surface_width: u32,
    surface_height: u32,
    viewport: GridRect,
    scale: FractionalScale,
    background: Color,
) {
    let viewport_x = scale.physical_length(viewport.x());
    let viewport_y = scale.physical_length(viewport.y());
    let (viewport_width, viewport_height) = viewport.size();
    let viewport_width = scale.physical_length(viewport_width);
    let viewport_height = scale.physical_length(viewport_height);
    let viewport_right = viewport_x.saturating_add(viewport_width).min(surface_width);
    let viewport_bottom = viewport_y
        .saturating_add(viewport_height)
        .min(surface_height);

    fill_rect(
        pixels,
        surface_width,
        Rect::new(0, 0, surface_width, viewport_y.min(surface_height)),
        background,
    );
    fill_rect(
        pixels,
        surface_width,
        Rect::new(
            0,
            viewport_bottom,
            surface_width,
            surface_height.saturating_sub(viewport_bottom),
        ),
        background,
    );
    fill_rect(
        pixels,
        surface_width,
        Rect::new(
            0,
            viewport_y,
            viewport_x.min(surface_width),
            viewport_bottom.saturating_sub(viewport_y),
        ),
        background,
    );
    fill_rect(
        pixels,
        surface_width,
        Rect::new(
            viewport_right,
            viewport_y,
            surface_width.saturating_sub(viewport_right),
            viewport_bottom.saturating_sub(viewport_y),
        ),
        background,
    );
}

fn apply_opacity(pixels: &mut [u32], opacity: u8) {
    if opacity == u8::MAX {
        return;
    }
    let scale = |channel: u8| {
        u8::try_from(u16::from(channel) * u16::from(opacity) / u16::from(u8::MAX))
            .unwrap_or(u8::MAX)
    };
    for pixel in pixels {
        let color = Color(*pixel);
        *pixel = Color::rgba(
            scale(color.r()),
            scale(color.g()),
            scale(color.b()),
            scale(color.a()),
        )
        .0;
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

#[cfg(test)]
mod card_geometry_tests {
    use super::card_geometry;

    #[test]
    fn card_geometry_prioritizes_thumbnail_content_over_metadata() {
        let geometry = card_geometry(400, 300);

        assert!(geometry.thumbnail_padding <= 7);
        assert!(geometry.footer_height <= 50);
        assert!(geometry.icon_size <= 34);
        assert!(geometry.title_font_size <= 14.0);
    }
}
