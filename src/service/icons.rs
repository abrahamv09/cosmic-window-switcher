// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashMap, path::Path};

use cosmic_window_switcher::ApplicationIcon;
use image::imageops::FilterType;

#[derive(Clone)]
pub(super) struct IconImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl IconImage {
    pub(super) const fn width(&self) -> u32 {
        self.width
    }

    pub(super) const fn height(&self) -> u32 {
        self.height
    }

    pub(super) fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Default)]
pub(super) struct IconResolver {
    cache: HashMap<(String, u32), Option<IconImage>>,
}

impl IconResolver {
    pub(super) fn resolve(
        &mut self,
        icon: &ApplicationIcon,
        target_size: u32,
    ) -> Option<&IconImage> {
        let key = (icon.name().to_owned(), target_size);
        self.cache
            .entry(key)
            .or_insert_with(|| load_icon(icon.name(), target_size))
            .as_ref()
    }
}

fn load_icon(name: &str, target_size: u32) -> Option<IconImage> {
    if name.trim().is_empty() {
        return None;
    }
    let size = u16::try_from(target_size).unwrap_or(u16::MAX);
    let path = freedesktop_icons::lookup(name)
        .with_size(size)
        .with_cache()
        .find()
        .or_else(|| {
            let lowercase = name.to_lowercase();
            freedesktop_icons::lookup(&lowercase)
                .with_size(size)
                .with_cache()
                .find()
        })?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("svg") => {
            load_svg_icon(&path, target_size)
        }
        _ => load_raster_icon(&path, target_size),
    }
}

fn load_raster_icon(path: &Path, target_size: u32) -> Option<IconImage> {
    let image = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .resize(target_size, target_size, FilterType::Lanczos3)
        .into_rgba8();
    Some(IconImage {
        width: image.width(),
        height: image.height(),
        pixels: image.into_raw(),
    })
}

fn load_svg_icon(path: &Path, target_size: u32) -> Option<IconImage> {
    let data = std::fs::read(path).ok()?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let target_size_f32 = f32::from(u16::try_from(target_size).ok()?);
    let scale = (target_size_f32 / size.width()).min(target_size_f32 / size.height());
    let rendered_width = size.width() * scale;
    let rendered_height = size.height() * scale;
    let transform = resvg::tiny_skia::Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        (target_size_f32 - rendered_width) / 2.0,
        (target_size_f32 - rendered_height) / 2.0,
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_size, target_size)?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut pixels = pixmap.take();
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3];
        if alpha == 0 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = u8::try_from((u16::from(*channel) * u16::from(u8::MAX)) / u16::from(alpha))
                .unwrap_or(u8::MAX);
        }
    }
    Some(IconImage {
        width: target_size,
        height: target_size,
        pixels,
    })
}
