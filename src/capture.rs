// SPDX-License-Identifier: GPL-3.0-only

use std::{error::Error, fmt, time::Duration};

use crate::WindowId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureBackend {
    DmaBuf,
    SharedMemory,
}

impl CaptureBackend {
    #[must_use]
    pub const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::DmaBuf => "dma-buf",
            Self::SharedMemory => "shared-memory",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmaBufFallbackReason {
    IncompatibleDevice,
    UnsupportedFormat,
    UnsupportedModifier,
    AllocationFailed,
    SynchronizationUnavailable,
    ImportFailed,
    ReleaseUnavailable,
}

impl DmaBufFallbackReason {
    #[must_use]
    pub const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::IncompatibleDevice => "incompatible-device",
            Self::UnsupportedFormat => "unsupported-format",
            Self::UnsupportedModifier => "unsupported-modifier",
            Self::AllocationFailed => "allocation-failed",
            Self::SynchronizationUnavailable => "synchronization-unavailable",
            Self::ImportFailed => "import-failed",
            Self::ReleaseUnavailable => "release-unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureBackendSelection {
    backend: CaptureBackend,
    fallback_reason: Option<DmaBufFallbackReason>,
}

impl Default for CaptureBackendSelection {
    fn default() -> Self {
        Self::shared_memory(DmaBufFallbackReason::ImportFailed)
    }
}

impl CaptureBackendSelection {
    #[must_use]
    pub const fn shared_memory(reason: DmaBufFallbackReason) -> Self {
        Self {
            backend: CaptureBackend::SharedMemory,
            fallback_reason: Some(reason),
        }
    }

    #[must_use]
    pub const fn backend(self) -> CaptureBackend {
        self.backend
    }

    #[must_use]
    pub const fn fallback_reason(self) -> Option<DmaBufFallbackReason> {
        self.fallback_reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmaBufContractStatus {
    Compatible,
    Incompatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmaBufCompatibility {
    pub device: DmaBufContractStatus,
    pub format: DmaBufContractStatus,
    pub modifier: DmaBufContractStatus,
    pub allocation: DmaBufContractStatus,
    pub synchronization: DmaBufContractStatus,
    pub import: DmaBufContractStatus,
    pub release: DmaBufContractStatus,
}

impl DmaBufCompatibility {
    #[must_use]
    pub const fn complete() -> Self {
        Self {
            device: DmaBufContractStatus::Compatible,
            format: DmaBufContractStatus::Compatible,
            modifier: DmaBufContractStatus::Compatible,
            allocation: DmaBufContractStatus::Compatible,
            synchronization: DmaBufContractStatus::Compatible,
            import: DmaBufContractStatus::Compatible,
            release: DmaBufContractStatus::Compatible,
        }
    }

    #[must_use]
    pub fn select_backend(self) -> CaptureBackendSelection {
        let fallback_reason = if self.device == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::IncompatibleDevice)
        } else if self.format == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::UnsupportedFormat)
        } else if self.modifier == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::UnsupportedModifier)
        } else if self.allocation == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::AllocationFailed)
        } else if self.synchronization == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::SynchronizationUnavailable)
        } else if self.import == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::ImportFailed)
        } else if self.release == DmaBufContractStatus::Incompatible {
            Some(DmaBufFallbackReason::ReleaseUnavailable)
        } else {
            None
        };
        match fallback_reason {
            Some(reason) => CaptureBackendSelection::shared_memory(reason),
            None => CaptureBackendSelection {
                backend: CaptureBackend::DmaBuf,
                fallback_reason: None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShmFormat {
    Abgr8888,
    Argb8888,
    Xbgr8888,
    Xrgb8888,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShmConstraints {
    pub width: u32,
    pub height: u32,
    pub formats: Vec<ShmFormat>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShmFrameLayout {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub byte_len: usize,
    pub format: ShmFormat,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BufferTransform {
    #[default]
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

#[derive(Clone, Copy)]
struct TransformGeometry {
    presentation_size: (u32, u32),
    raw_x: (i32, i32, u32),
    raw_y: (i32, i32, u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailFrame {
    layout: ShmFrameLayout,
    pixels: Vec<u8>,
    transform: BufferTransform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidThumbnailFrame {
    expected: usize,
    actual: usize,
}

impl fmt::Display for InvalidThumbnailFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "thumbnail frame has {} bytes; expected exactly {}",
            self.actual, self.expected
        )
    }
}

impl Error for InvalidThumbnailFrame {}

impl ThumbnailFrame {
    /// Takes ownership of one exact-size, in-memory SHM frame.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidThumbnailFrame`] when the pixel allocation does not
    /// match the compositor-negotiated layout.
    pub fn new(layout: ShmFrameLayout, pixels: Vec<u8>) -> Result<Self, InvalidThumbnailFrame> {
        Self::with_transform(layout, pixels, BufferTransform::Normal)
    }

    /// Takes ownership of one exact-size SHM frame with its compositor transform.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidThumbnailFrame`] when the pixel allocation does not
    /// match the compositor-negotiated layout.
    pub fn with_transform(
        layout: ShmFrameLayout,
        pixels: Vec<u8>,
        transform: BufferTransform,
    ) -> Result<Self, InvalidThumbnailFrame> {
        if pixels.len() != layout.byte_len {
            return Err(InvalidThumbnailFrame {
                expected: layout.byte_len,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            layout,
            pixels,
            transform,
        })
    }

    #[must_use]
    pub const fn layout(&self) -> ShmFrameLayout {
        self.layout
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    #[must_use]
    pub fn argb_pixel(&self, x: u32, y: u32) -> Option<u32> {
        let geometry = self.transform_geometry();
        let (presentation_width, presentation_height) = geometry.presentation_size;
        if x >= presentation_width || y >= presentation_height {
            return None;
        }
        let (raw_x, raw_y) = geometry.raw_coordinates(x, y);
        let offset = usize::try_from(raw_y)
            .ok()?
            .checked_mul(usize::try_from(self.layout.stride).ok()?)?
            .checked_add(usize::try_from(raw_x).ok()?.checked_mul(4)?)?;
        let packed = u32::from_ne_bytes(self.pixels.get(offset..offset + 4)?.try_into().ok()?);
        Some(match self.layout.format {
            ShmFormat::Argb8888 => packed,
            ShmFormat::Xrgb8888 => 0xFF00_0000 | (packed & 0x00FF_FFFF),
            ShmFormat::Abgr8888 => swap_red_and_blue(packed),
            ShmFormat::Xbgr8888 => 0xFF00_0000 | swap_red_and_blue(packed),
        })
    }

    #[must_use]
    pub const fn presentation_size(&self) -> (u32, u32) {
        self.transform_geometry().presentation_size
    }

    #[must_use]
    pub fn fitted_size(&self, maximum_width: u32, maximum_height: u32) -> (u32, u32) {
        let (presentation_width, presentation_height) = self.presentation_size();
        if presentation_width == 0
            || presentation_height == 0
            || maximum_width == 0
            || maximum_height == 0
        {
            return (0, 0);
        }
        let source_width = u64::from(presentation_width);
        let source_height = u64::from(presentation_height);
        let maximum_width_u64 = u64::from(maximum_width);
        let maximum_height_u64 = u64::from(maximum_height);
        if source_width * maximum_height_u64 > maximum_width_u64 * source_height {
            let height = source_height * maximum_width_u64 / source_width;
            (
                maximum_width,
                u32::try_from(height).unwrap_or(maximum_height).max(1),
            )
        } else {
            let width = source_width * maximum_height_u64 / source_height;
            (
                u32::try_from(width).unwrap_or(maximum_width).max(1),
                maximum_height,
            )
        }
    }

    const fn transform_geometry(&self) -> TransformGeometry {
        let width = self.layout.width;
        let height = self.layout.height;
        let maximum_x = width.saturating_sub(1);
        let maximum_y = height.saturating_sub(1);
        match self.transform {
            BufferTransform::Normal => {
                TransformGeometry::new((width, height), (1, 0, 0), (0, 1, 0))
            }
            BufferTransform::Rotate90 => {
                TransformGeometry::new((height, width), (0, -1, maximum_x), (1, 0, 0))
            }
            BufferTransform::Rotate180 => {
                TransformGeometry::new((width, height), (-1, 0, maximum_x), (0, -1, maximum_y))
            }
            BufferTransform::Rotate270 => {
                TransformGeometry::new((height, width), (0, 1, 0), (-1, 0, maximum_y))
            }
            BufferTransform::Flipped => {
                TransformGeometry::new((width, height), (-1, 0, maximum_x), (0, 1, 0))
            }
            BufferTransform::Flipped90 => {
                TransformGeometry::new((height, width), (0, 1, 0), (1, 0, 0))
            }
            BufferTransform::Flipped180 => {
                TransformGeometry::new((width, height), (1, 0, 0), (0, -1, maximum_y))
            }
            BufferTransform::Flipped270 => {
                TransformGeometry::new((height, width), (0, -1, maximum_x), (-1, 0, maximum_y))
            }
        }
    }
}

impl TransformGeometry {
    const fn new(
        presentation_size: (u32, u32),
        raw_x: (i32, i32, u32),
        raw_y: (i32, i32, u32),
    ) -> Self {
        Self {
            presentation_size,
            raw_x,
            raw_y,
        }
    }

    fn raw_coordinates(self, x: u32, y: u32) -> (u32, u32) {
        (
            apply_coordinate(self.raw_x, x, y),
            apply_coordinate(self.raw_y, x, y),
        )
    }
}

fn apply_coordinate((x_scale, y_scale, offset): (i32, i32, u32), x: u32, y: u32) -> u32 {
    let coordinate =
        i64::from(x_scale) * i64::from(x) + i64::from(y_scale) * i64::from(y) + i64::from(offset);
    u32::try_from(coordinate).expect("validated transformed coordinates remain in the SHM frame")
}

const fn swap_red_and_blue(packed: u32) -> u32 {
    let alpha = packed & 0xFF00_0000;
    let red = packed & 0x0000_00FF;
    let green = packed & 0x0000_FF00;
    let blue = packed & 0x00FF_0000;
    alpha | (red << 16) | green | (blue >> 16)
}

impl ShmConstraints {
    #[must_use]
    pub fn negotiate(&self) -> Option<ShmFrameLayout> {
        let format = [
            ShmFormat::Argb8888,
            ShmFormat::Abgr8888,
            ShmFormat::Xrgb8888,
            ShmFormat::Xbgr8888,
        ]
        .into_iter()
        .find(|candidate| self.formats.contains(candidate))?;
        let stride = self.width.checked_mul(4)?;
        let byte_len = usize::try_from(stride)
            .ok()?
            .checked_mul(usize::try_from(self.height).ok()?)?;
        (self.width > 0
            && self.height > 0
            && i32::try_from(self.width).is_ok()
            && i32::try_from(self.height).is_ok()
            && i32::try_from(stride).is_ok()
            && i32::try_from(byte_len).is_ok())
        .then_some(ShmFrameLayout {
            width: self.width,
            height: self.height,
            stride,
            byte_len,
            format,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameDamage {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl FrameDamage {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq, Eq, serde::Serialize)]
pub enum RefreshCeiling {
    Fps15,
    Fps30,
    Fps60,
    MatchDisplay,
}

impl RefreshCeiling {
    fn interval(self, display_refresh_rate: u16) -> Duration {
        let frames_per_second = match self {
            Self::Fps15 => 15,
            Self::Fps30 => 30,
            Self::Fps60 => 60,
            Self::MatchDisplay => u64::from(display_refresh_rate.max(1)),
        };
        Duration::from_nanos(1_000_000_000 / frames_per_second)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureFailure {
    FrameFailed,
    Stopped,
    ProtectedContent,
    UnsupportedXWayland,
    UnsupportedFormat,
    InvalidDimensions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureEffect {
    CreateStream(WindowId),
    RequestFrame {
        window: WindowId,
        layout: ShmFrameLayout,
    },
    PresentThumbnail(WindowId),
    DegradeThumbnail {
        window: WindowId,
        reason: CaptureFailure,
    },
    ReleaseStream(WindowId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureOpportunity {
    InputPending,
    InputDrained,
}

#[derive(Clone, Debug)]
struct CaptureStream {
    window: WindowId,
    layout: Option<ShmFrameLayout>,
    outstanding: bool,
    next_request_at: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct CaptureSessionModel {
    refresh_ceiling: RefreshCeiling,
    display_refresh_rate: u16,
    streams: Vec<CaptureStream>,
    degraded: Vec<WindowId>,
    selected: Option<WindowId>,
    background_cursor: usize,
}

impl CaptureSessionModel {
    #[must_use]
    pub const fn new(refresh_ceiling: RefreshCeiling) -> Self {
        Self {
            refresh_ceiling,
            display_refresh_rate: 60,
            streams: Vec::new(),
            degraded: Vec::new(),
            selected: None,
            background_cursor: 0,
        }
    }

    pub fn set_display_refresh_rate(&mut self, frames_per_second: u16) {
        self.display_refresh_rate = frames_per_second.max(1);
    }

    pub fn set_visible(
        &mut self,
        windows: impl IntoIterator<Item = WindowId>,
    ) -> Vec<CaptureEffect> {
        let windows = windows.into_iter().collect::<Vec<_>>();
        let mut effects = Vec::new();
        self.degraded.retain(|window| windows.contains(window));
        self.streams.retain(|stream| {
            let retained = windows.contains(&stream.window);
            if !retained {
                effects.push(CaptureEffect::ReleaseStream(stream.window.clone()));
            }
            retained
        });
        self.background_cursor = self
            .background_cursor
            .min(self.streams.len().saturating_sub(1));
        for window in windows {
            if self.streams.iter().any(|stream| stream.window == window)
                || self.degraded.contains(&window)
            {
                continue;
            }
            effects.push(CaptureEffect::CreateStream(window.clone()));
            self.streams.push(CaptureStream {
                window,
                layout: None,
                outstanding: false,
                next_request_at: None,
            });
        }
        effects
    }

    pub fn set_selected(&mut self, selected: Option<WindowId>) {
        self.selected = selected;
    }

    pub fn initialized(
        &mut self,
        window: &WindowId,
        constraints: &ShmConstraints,
    ) -> Vec<CaptureEffect> {
        let Some(stream) = self
            .streams
            .iter_mut()
            .find(|stream| stream.window == *window)
        else {
            return Vec::new();
        };
        let Some(layout) = constraints.negotiate() else {
            return self.failed(
                window,
                if constraints.formats.is_empty() {
                    CaptureFailure::UnsupportedFormat
                } else {
                    CaptureFailure::InvalidDimensions
                },
            );
        };
        stream.layout = Some(layout);
        if stream.outstanding {
            return Vec::new();
        }
        stream.next_request_at = Some(Duration::ZERO);
        Vec::new()
    }

    pub fn frame_ready(
        &mut self,
        window: &WindowId,
        now: Duration,
        damage: &[FrameDamage],
    ) -> Vec<CaptureEffect> {
        let Some(stream) = self
            .streams
            .iter_mut()
            .find(|stream| stream.window == *window)
        else {
            return Vec::new();
        };
        if !stream.outstanding {
            return Vec::new();
        }
        stream.outstanding = false;
        stream.next_request_at =
            Some(now + self.refresh_ceiling.interval(self.display_refresh_rate));
        if damage.is_empty() {
            Vec::new()
        } else {
            vec![CaptureEffect::PresentThumbnail(window.clone())]
        }
    }

    pub fn refresh_due(
        &mut self,
        now: Duration,
        opportunity: CaptureOpportunity,
    ) -> Vec<CaptureEffect> {
        if opportunity == CaptureOpportunity::InputPending {
            return Vec::new();
        }
        let mut effects = Vec::new();
        let selected_index = self.selected.as_ref().and_then(|selected| {
            self.streams
                .iter()
                .position(|stream| stream.window == *selected)
        });
        if let Some(index) = selected_index
            && let Some(effect) = request_due_frame(&mut self.streams[index], now)
        {
            effects.push(effect);
        }

        let stream_count = self.streams.len();
        for offset in 0..stream_count {
            let index = (self.background_cursor + offset) % stream_count;
            if Some(index) == selected_index {
                continue;
            }
            let Some(effect) = request_due_frame(&mut self.streams[index], now) else {
                continue;
            };
            effects.push(effect);
            self.background_cursor = (index + 1) % stream_count;
            break;
        }
        effects
    }

    pub fn failed(&mut self, window: &WindowId, reason: CaptureFailure) -> Vec<CaptureEffect> {
        let Some(index) = self
            .streams
            .iter()
            .position(|stream| stream.window == *window)
        else {
            return Vec::new();
        };
        self.streams.remove(index);
        if !self.degraded.contains(window) {
            self.degraded.push(window.clone());
        }
        vec![CaptureEffect::DegradeThumbnail {
            window: window.clone(),
            reason,
        }]
    }

    pub fn window_closed(&mut self, window: &WindowId) -> Vec<CaptureEffect> {
        self.degraded.retain(|degraded| degraded != window);
        let Some(index) = self
            .streams
            .iter()
            .position(|stream| stream.window == *window)
        else {
            return Vec::new();
        };
        let stream = self.streams.remove(index);
        vec![CaptureEffect::ReleaseStream(stream.window)]
    }

    pub fn stop(&mut self) -> Vec<CaptureEffect> {
        self.degraded.clear();
        self.background_cursor = 0;
        self.streams
            .drain(..)
            .map(|stream| CaptureEffect::ReleaseStream(stream.window))
            .collect()
    }

    #[must_use]
    pub fn is_active(&self, window: &WindowId) -> bool {
        self.streams.iter().any(|stream| stream.window == *window)
    }

    #[must_use]
    pub const fn active_stream_count(&self) -> usize {
        self.streams.len()
    }

    #[must_use]
    pub fn next_request_at(&self) -> Option<Duration> {
        self.streams
            .iter()
            .filter(|stream| !stream.outstanding)
            .filter_map(|stream| stream.next_request_at)
            .min()
    }
}

fn request_due_frame(stream: &mut CaptureStream, now: Duration) -> Option<CaptureEffect> {
    if stream.outstanding
        || stream
            .next_request_at
            .is_none_or(|next_request_at| next_request_at > now)
    {
        return None;
    }
    let layout = stream.layout?;
    stream.outstanding = true;
    stream.next_request_at = None;
    Some(CaptureEffect::RequestFrame {
        window: stream.window.clone(),
        layout,
    })
}
