// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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
    let path = find_icon(name, size).or_else(|| {
        desktop_icon_name(name).and_then(|canonical_name| find_icon(&canonical_name, size))
    })?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("svg") => {
            load_svg_icon(&path, target_size)
        }
        _ => load_raster_icon(&path, target_size),
    }
}

fn find_icon(name: &str, size: u16) -> Option<PathBuf> {
    freedesktop_icons::lookup(name)
        .with_size(size)
        .with_cache()
        .find()
        .or_else(|| {
            let lowercase = name.to_lowercase();
            freedesktop_icons::lookup(&lowercase)
                .with_size(size)
                .with_cache()
                .find()
        })
}

fn desktop_icon_name(application_id: &str) -> Option<String> {
    desktop_application_directories()
        .into_iter()
        .filter_map(|directory| std::fs::read_dir(directory).ok())
        .flatten()
        .filter_map(Result::ok)
        .find_map(|entry| {
            let path = entry.path();
            let desktop_id = path.file_stem().and_then(|file_name| file_name.to_str())?;
            let contents = std::fs::read_to_string(&path).ok()?;
            desktop_entry_icon_name(application_id, desktop_id, &contents)
        })
}

fn desktop_application_directories() -> Vec<PathBuf> {
    let mut data_directories = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        data_directories.push(data_home);
    } else if let Some(home) = std::env::var_os("HOME") {
        data_directories.push(PathBuf::from(home).join(".local/share"));
    }
    data_directories.extend(std::env::var_os("XDG_DATA_DIRS").map_or_else(
        || {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        },
        |directories| std::env::split_paths(&directories).collect::<Vec<_>>(),
    ));
    data_directories
        .into_iter()
        .map(|directory| directory.join("applications"))
        .collect()
}

fn desktop_entry_icon_name(
    application_id: &str,
    desktop_id: &str,
    contents: &str,
) -> Option<String> {
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut icon = None;
    let mut startup_wm_class = None;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Name" if name.is_none() => name = Some(value),
            "Icon" if icon.is_none() => icon = Some(value),
            "StartupWMClass" if startup_wm_class.is_none() => {
                startup_wm_class = Some(value);
            }
            _ => {}
        }
    }

    let application_id = application_id.trim();
    let desktop_suffix = desktop_id.rsplit('.').next().unwrap_or(desktop_id);
    let matches = desktop_id.eq_ignore_ascii_case(application_id)
        || desktop_suffix.eq_ignore_ascii_case(application_id)
        || name.is_some_and(|name| name.eq_ignore_ascii_case(application_id))
        || startup_wm_class
            .is_some_and(|startup_wm_class| startup_wm_class.eq_ignore_ascii_case(application_id));
    matches.then(|| icon.unwrap_or(desktop_id).to_owned())
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

#[cfg(test)]
mod tests {
    use super::desktop_entry_icon_name;

    #[test]
    fn compositor_aliases_resolve_through_desktop_entry_names_and_ids() {
        assert_eq!(
            desktop_entry_icon_name(
                "vlc",
                "org.videolan.VLC",
                "[Desktop Entry]\nName=VLC media player\nIcon=org.videolan.VLC\n",
            )
            .as_deref(),
            Some("org.videolan.VLC")
        );
        assert_eq!(
            desktop_entry_icon_name(
                "MongoDB Compass",
                "com.mongodb.Compass",
                "[Desktop Entry]\nName=MongoDB Compass\nIcon=com.mongodb.Compass\n",
            )
            .as_deref(),
            Some("com.mongodb.Compass")
        );
    }
}
