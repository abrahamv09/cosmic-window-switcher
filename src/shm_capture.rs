// SPDX-License-Identifier: GPL-3.0-only

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use cosmic_client_toolkit::GlobalData;
use cosmic_window_switcher::{
    BufferTransform, FrameDamage, ShmConstraints, ShmFormat, ShmFrameLayout, ThumbnailFrame,
    WindowId,
};
use smithay_client_toolkit::shm::{Shm, raw::RawPool};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    globals::GlobalList,
    protocol::{wl_buffer, wl_output, wl_shm},
};
use wayland_protocols::ext::{
    foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1,
    image_capture_source::v1::client::{
        ext_foreign_toplevel_image_capture_source_manager_v1, ext_image_capture_source_v1,
    },
    image_copy_capture::v1::client::{
        ext_image_copy_capture_frame_v1, ext_image_copy_capture_manager_v1,
        ext_image_copy_capture_session_v1,
    },
};

pub(crate) type CaptureSession = ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
pub(crate) type CaptureFrame = ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1;

pub(crate) trait ShmCaptureHandler: Sized {
    fn constraints_ready(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        session: &CaptureSession,
        constraints: ShmConstraints,
    );
    fn capture_stopped(&mut self, session: &CaptureSession);
    fn frame_ready(&mut self, frame: &CaptureFrame);
    fn frame_failed(&mut self, frame: &CaptureFrame);
}

#[derive(Default)]
struct PendingConstraints {
    size: (u32, u32),
    formats: Vec<ShmFormat>,
}

pub(crate) struct CaptureSessionData {
    window: WindowId,
    constraints: Mutex<PendingConstraints>,
}

#[derive(Default)]
struct FrameMetadata {
    transform: BufferTransform,
    damage: Vec<FrameDamage>,
}

pub(crate) struct CaptureFrameData {
    window: WindowId,
    allocation: Arc<ShmAllocation>,
    metadata: Mutex<FrameMetadata>,
}

struct ShmAllocation {
    pool: Mutex<Option<RawPool>>,
    buffer: wl_buffer::WlBuffer,
    layout: ShmFrameLayout,
}

impl ShmAllocation {
    fn pixels(&self) -> Option<Vec<u8>> {
        let mut pool = self
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Some(pool.as_mut()?.mmap().to_vec())
    }

    fn release(&self) {
        let mut pool = self
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pool.take().is_some() {
            self.buffer.destroy();
        }
    }
}

impl Drop for ShmAllocation {
    fn drop(&mut self) {
        let pool = self
            .pool
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pool.take().is_some() {
            self.buffer.destroy();
        }
    }
}

struct CaptureStream {
    window: WindowId,
    session: CaptureSession,
    frame: Option<CaptureFrame>,
    allocation: Option<Arc<ShmAllocation>>,
}

pub(crate) struct CompletedFrame {
    pub(crate) window: WindowId,
    pub(crate) thumbnail: ThumbnailFrame,
    pub(crate) damage: Vec<FrameDamage>,
}

pub(crate) struct ShmCaptureState {
    copy_manager: Option<ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1>,
    source_manager:
        Option<ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1>,
    streams: Vec<CaptureStream>,
}

impl ShmCaptureState {
    pub(crate) fn new<D>(globals: &GlobalList, queue_handle: &QueueHandle<D>) -> Self
    where
        D: Dispatch<ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1, GlobalData>
            + Dispatch<
                ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
                GlobalData,
            >
            + 'static,
    {
        Self {
            copy_manager: globals.bind(queue_handle, 1..=1, GlobalData).ok(),
            source_manager: globals.bind(queue_handle, 1..=1, GlobalData).ok(),
            streams: Vec::new(),
        }
    }

    #[must_use]
    pub(crate) const fn contract_available(&self) -> bool {
        self.copy_manager.is_some() && self.source_manager.is_some()
    }

    pub(crate) fn create_stream<D>(
        &mut self,
        window: WindowId,
        toplevel: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        queue_handle: &QueueHandle<D>,
    ) -> Result<()>
    where
        D: Dispatch<ext_image_capture_source_v1::ExtImageCaptureSourceV1, GlobalData>
            + Dispatch<CaptureSession, CaptureSessionData>
            + 'static,
    {
        if self.streams.iter().any(|stream| stream.window == window) {
            return Ok(());
        }
        let source_manager = self
            .source_manager
            .as_ref()
            .context("foreign-toplevel image capture source protocol is unavailable")?;
        let copy_manager = self
            .copy_manager
            .as_ref()
            .context("image-copy-capture protocol is unavailable")?;
        let source = source_manager.create_source(toplevel, queue_handle, GlobalData);
        let session = copy_manager.create_session(
            &source,
            ext_image_copy_capture_manager_v1::Options::empty(),
            queue_handle,
            CaptureSessionData {
                window: window.clone(),
                constraints: Mutex::new(PendingConstraints::default()),
            },
        );
        source.destroy();
        self.streams.push(CaptureStream {
            window,
            session,
            frame: None,
            allocation: None,
        });
        Ok(())
    }

    pub(crate) fn request_frame<D>(
        &mut self,
        window: &WindowId,
        layout: ShmFrameLayout,
        shm: &Shm,
        queue_handle: &QueueHandle<D>,
    ) -> Result<()>
    where
        D: Dispatch<wl_buffer::WlBuffer, ()> + Dispatch<CaptureFrame, CaptureFrameData> + 'static,
    {
        let stream = self
            .streams
            .iter_mut()
            .find(|stream| stream.window == *window)
            .context("request a frame for an absent capture stream")?;
        if stream.frame.is_some() {
            bail!("a capture stream already has one frame outstanding");
        }
        let width = i32::try_from(layout.width).context("Live Thumbnail width is too large")?;
        let height = i32::try_from(layout.height).context("Live Thumbnail height is too large")?;
        let (allocation, buffer_damage) = if let Some(allocation) = stream
            .allocation
            .as_ref()
            .filter(|allocation| allocation.layout == layout)
            .cloned()
        {
            (allocation, Vec::new())
        } else {
            if let Some(previous) = stream.allocation.take() {
                previous.release();
            }
            let mut pool = RawPool::new(layout.byte_len, shm)
                .context("allocate memory-only Live Thumbnail frame")?;
            let stride =
                i32::try_from(layout.stride).context("Live Thumbnail stride is too large")?;
            let buffer = pool.create_buffer(
                0,
                width,
                height,
                stride,
                wl_shm_format(layout.format),
                (),
                queue_handle,
            );
            (
                Arc::new(ShmAllocation {
                    pool: Mutex::new(Some(pool)),
                    buffer,
                    layout,
                }),
                vec![(0, 0, width, height)],
            )
        };
        let frame = stream.session.create_frame(
            queue_handle,
            CaptureFrameData {
                window: window.clone(),
                allocation: Arc::clone(&allocation),
                metadata: Mutex::new(FrameMetadata::default()),
            },
        );
        frame.attach_buffer(&allocation.buffer);
        for (x, y, width, height) in buffer_damage {
            frame.damage_buffer(x, y, width, height);
        }
        frame.capture();
        stream.allocation = Some(allocation);
        stream.frame = Some(frame);
        Ok(())
    }

    pub(crate) fn complete_frame(&mut self, frame: &CaptureFrame) -> Result<CompletedFrame> {
        let completed = (|| {
            let data = frame
                .data::<CaptureFrameData>()
                .context("capture completed without frame metadata")?;
            let metadata = data
                .metadata
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let pixels = data
                .allocation
                .pixels()
                .context("capture allocation was released before frame completion")?;
            let thumbnail =
                ThumbnailFrame::with_transform(data.allocation.layout, pixels, metadata.transform)
                    .context("capture returned a non-exact SHM frame")?;
            Ok(CompletedFrame {
                window: data.window.clone(),
                thumbnail,
                damage: metadata.damage.clone(),
            })
        })();
        self.destroy_frame(frame);
        completed
    }

    pub(crate) fn frame_window(frame: &CaptureFrame) -> Option<WindowId> {
        frame
            .data::<CaptureFrameData>()
            .map(|data| data.window.clone())
    }

    pub(crate) fn session_window(session: &CaptureSession) -> Option<WindowId> {
        session
            .data::<CaptureSessionData>()
            .map(|data| data.window.clone())
    }

    pub(crate) fn stop_stream(&mut self, window: &WindowId) {
        let Some(index) = self
            .streams
            .iter()
            .position(|stream| stream.window == *window)
        else {
            return;
        };
        let mut stream = self.streams.remove(index);
        if let Some(frame) = stream.frame.take() {
            frame.destroy();
        }
        if let Some(allocation) = stream.allocation.take() {
            allocation.release();
        }
        stream.session.destroy();
    }

    pub(crate) fn stop_all(&mut self) -> (usize, usize, usize) {
        let session_count = self.streams.len();
        let frame_count = self
            .streams
            .iter()
            .filter(|stream| stream.frame.is_some())
            .count();
        let allocation_count = self
            .streams
            .iter()
            .filter(|stream| stream.allocation.is_some())
            .count();
        let windows = self
            .streams
            .iter()
            .map(|stream| stream.window.clone())
            .collect::<Vec<_>>();
        for window in windows {
            self.stop_stream(&window);
        }
        (session_count, frame_count, allocation_count)
    }

    pub(crate) fn fail_frame(&mut self, frame: &CaptureFrame) {
        self.destroy_frame(frame);
    }

    fn destroy_frame(&mut self, frame: &CaptureFrame) {
        if let Some(stream) = self
            .streams
            .iter_mut()
            .find(|stream| stream.frame.as_ref() == Some(frame))
            && let Some(frame) = stream.frame.take()
        {
            frame.destroy();
            return;
        }
        frame.destroy();
    }
}

impl Drop for ShmCaptureState {
    fn drop(&mut self) {
        self.stop_all();
        if let Some(manager) = self.copy_manager.take() {
            manager.destroy();
        }
        if let Some(manager) = self.source_manager.take() {
            manager.destroy();
        }
    }
}

impl<D> Dispatch<ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1, GlobalData, D>
    for ShmCaptureState
where
    D: Dispatch<ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1, GlobalData>,
{
    fn event(
        _state: &mut D,
        _proxy: &ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
        _event: ext_image_copy_capture_manager_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<D>,
    ) {
        unreachable!();
    }
}

impl<D>
    Dispatch<
        ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
        GlobalData,
        D,
    > for ShmCaptureState
where
    D: Dispatch<
            ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
            GlobalData,
        >,
{
    fn event(
        _state: &mut D,
        _proxy: &ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
        _event: ext_foreign_toplevel_image_capture_source_manager_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<D>,
    ) {
        unreachable!();
    }
}

impl<D> Dispatch<ext_image_capture_source_v1::ExtImageCaptureSourceV1, GlobalData, D>
    for ShmCaptureState
where
    D: Dispatch<ext_image_capture_source_v1::ExtImageCaptureSourceV1, GlobalData>,
{
    fn event(
        _state: &mut D,
        _proxy: &ext_image_capture_source_v1::ExtImageCaptureSourceV1,
        _event: ext_image_capture_source_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<D>,
    ) {
        unreachable!();
    }
}

impl<D> Dispatch<CaptureSession, CaptureSessionData, D> for ShmCaptureState
where
    D: Dispatch<CaptureSession, CaptureSessionData> + ShmCaptureHandler,
{
    fn event(
        state: &mut D,
        session: &CaptureSession,
        event: ext_image_copy_capture_session_v1::Event,
        data: &CaptureSessionData,
        _connection: &Connection,
        queue_handle: &QueueHandle<D>,
    ) {
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                data.constraints
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .size = (width, height);
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat {
                format: WEnum::Value(format),
            } => {
                if let Some(format) = shm_format(format) {
                    data.constraints
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .formats
                        .push(format);
                }
            }
            ext_image_copy_capture_session_v1::Event::Done => {
                let constraints = data
                    .constraints
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.constraints_ready(
                    queue_handle,
                    session,
                    ShmConstraints {
                        width: constraints.size.0,
                        height: constraints.size.1,
                        formats: constraints.formats.clone(),
                    },
                );
            }
            ext_image_copy_capture_session_v1::Event::Stopped => {
                state.capture_stopped(session);
            }
            _ => {}
        }
    }
}

impl<D> Dispatch<CaptureFrame, CaptureFrameData, D> for ShmCaptureState
where
    D: Dispatch<CaptureFrame, CaptureFrameData> + ShmCaptureHandler,
{
    fn event(
        state: &mut D,
        frame: &CaptureFrame,
        event: ext_image_copy_capture_frame_v1::Event,
        data: &CaptureFrameData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<D>,
    ) {
        match event {
            ext_image_copy_capture_frame_v1::Event::Transform { transform } => {
                data.metadata
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .transform = buffer_transform(transform);
            }
            ext_image_copy_capture_frame_v1::Event::Damage {
                x,
                y,
                width,
                height,
            } => {
                data.metadata
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .damage
                    .push(FrameDamage::new(x, y, width, height));
            }
            ext_image_copy_capture_frame_v1::Event::Ready => state.frame_ready(frame),
            ext_image_copy_capture_frame_v1::Event::Failed { .. } => state.frame_failed(frame),
            _ => {}
        }
    }
}

fn shm_format(format: wl_shm::Format) -> Option<ShmFormat> {
    match format {
        wl_shm::Format::Abgr8888 => Some(ShmFormat::Abgr8888),
        wl_shm::Format::Argb8888 => Some(ShmFormat::Argb8888),
        wl_shm::Format::Xbgr8888 => Some(ShmFormat::Xbgr8888),
        wl_shm::Format::Xrgb8888 => Some(ShmFormat::Xrgb8888),
        _ => None,
    }
}

const fn wl_shm_format(format: ShmFormat) -> wl_shm::Format {
    match format {
        ShmFormat::Abgr8888 => wl_shm::Format::Abgr8888,
        ShmFormat::Argb8888 => wl_shm::Format::Argb8888,
        ShmFormat::Xbgr8888 => wl_shm::Format::Xbgr8888,
        ShmFormat::Xrgb8888 => wl_shm::Format::Xrgb8888,
    }
}

const fn buffer_transform(transform: WEnum<wl_output::Transform>) -> BufferTransform {
    match transform {
        WEnum::Value(wl_output::Transform::_90) => BufferTransform::Rotate90,
        WEnum::Value(wl_output::Transform::_180) => BufferTransform::Rotate180,
        WEnum::Value(wl_output::Transform::_270) => BufferTransform::Rotate270,
        WEnum::Value(wl_output::Transform::Flipped) => BufferTransform::Flipped,
        WEnum::Value(wl_output::Transform::Flipped90) => BufferTransform::Flipped90,
        WEnum::Value(wl_output::Transform::Flipped180) => BufferTransform::Flipped180,
        WEnum::Value(wl_output::Transform::Flipped270) => BufferTransform::Flipped270,
        WEnum::Unknown(_) | WEnum::Value(_) => BufferTransform::Normal,
    }
}

pub(crate) fn duration_to_timespec(duration: Duration) -> rustix::event::Timespec {
    rustix::event::Timespec {
        tv_sec: duration
            .as_secs()
            .try_into()
            .expect("a capture deadline fits in seconds"),
        tv_nsec: duration.subsec_nanos().into(),
    }
}
