// SPDX-License-Identifier: GPL-3.0-only

use std::time::Duration;

use cosmic_window_switcher::{
    BufferTransform, CaptureEffect, CaptureFailure, CaptureSessionModel, FrameDamage,
    RefreshCeiling, ShmConstraints, ShmFormat, ShmFrameLayout, SwitcherItem, ThumbnailFrame,
    WindowId,
};

fn window(id: &str) -> WindowId {
    WindowId::from(id)
}

fn constraints() -> ShmConstraints {
    ShmConstraints {
        width: 1_920,
        height: 1_080,
        formats: vec![ShmFormat::Xrgb8888, ShmFormat::Argb8888],
    }
}

#[test]
fn visible_windows_negotiate_exact_shm_frames_with_one_request_outstanding() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);

    assert_eq!(
        captures.set_visible([window("selected"), window("other")]),
        vec![
            CaptureEffect::CreateStream(window("selected")),
            CaptureEffect::CreateStream(window("other")),
        ]
    );
    assert_eq!(
        captures.initialized(&window("selected"), &constraints()),
        vec![CaptureEffect::RequestFrame {
            window: window("selected"),
            layout: ShmFrameLayout {
                width: 1_920,
                height: 1_080,
                stride: 7_680,
                byte_len: 8_294_400,
                format: ShmFormat::Argb8888,
            },
        }]
    );
    assert!(
        captures
            .initialized(&window("selected"), &constraints())
            .is_empty()
    );
}

#[test]
fn changed_content_refreshes_at_the_ceiling_while_unchanged_content_is_not_presented() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    captures.set_visible([window("selected")]);
    captures.set_selected(Some(window("selected")));
    captures.initialized(&window("selected"), &constraints());

    assert_eq!(
        captures.frame_ready(
            &window("selected"),
            Duration::ZERO,
            &[FrameDamage::new(0, 0, 1_920, 1_080)],
        ),
        vec![CaptureEffect::PresentThumbnail(window("selected"))]
    );
    assert!(captures.refresh_due(Duration::from_millis(32)).is_empty());
    assert_eq!(
        captures.refresh_due(Duration::from_millis(34)),
        vec![CaptureEffect::RequestFrame {
            window: window("selected"),
            layout: ShmFrameLayout {
                width: 1_920,
                height: 1_080,
                stride: 7_680,
                byte_len: 8_294_400,
                format: ShmFormat::Argb8888,
            },
        }]
    );
    assert!(
        captures
            .frame_ready(&window("selected"), Duration::from_millis(35), &[],)
            .is_empty()
    );
}

#[test]
fn one_window_failure_degrades_only_that_switcher_item() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    captures.set_visible([window("native"), window("xwayland")]);
    captures.initialized(&window("native"), &constraints());
    captures.initialized(&window("xwayland"), &constraints());

    assert_eq!(
        captures.failed(&window("xwayland"), CaptureFailure::ProtectedContent),
        vec![CaptureEffect::DegradeThumbnail {
            window: window("xwayland"),
            reason: CaptureFailure::ProtectedContent,
        }]
    );
    assert!(captures.is_active(&window("native")));
    assert!(!captures.is_active(&window("xwayland")));
}

#[test]
fn window_closure_viewport_exit_and_session_stop_release_every_stream() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    captures.set_visible([window("one"), window("two"), window("three")]);

    assert_eq!(
        captures.window_closed(&window("two")),
        vec![CaptureEffect::ReleaseStream(window("two"))]
    );
    assert_eq!(
        captures.set_visible([window("three")]),
        vec![CaptureEffect::ReleaseStream(window("one"))]
    );
    assert_eq!(
        captures.stop(),
        vec![CaptureEffect::ReleaseStream(window("three"))]
    );
    assert_eq!(captures.active_stream_count(), 0);
}

#[test]
fn full_window_content_fits_without_cropping_or_distortion() {
    let frame = ThumbnailFrame::new(
        ShmFrameLayout {
            width: 1_920,
            height: 1_080,
            stride: 7_680,
            byte_len: 8_294_400,
            format: ShmFormat::Argb8888,
        },
        vec![0; 8_294_400],
    )
    .expect("the exact frame is valid");

    assert_eq!(frame.fitted_size(160, 100), (160, 90));
    assert_eq!(frame.fitted_size(100, 160), (100, 56));
}

#[test]
fn capture_failure_keeps_the_switcher_items_icon_and_title_fallback() {
    let mut item = SwitcherItem::new(
        window("protected"),
        "org.example.Player".to_owned(),
        "Protected video".to_owned(),
    );
    item.update_thumbnail(
        ThumbnailFrame::new(
            ShmFrameLayout {
                width: 1,
                height: 1,
                stride: 4,
                byte_len: 4,
                format: ShmFormat::Xrgb8888,
            },
            vec![0; 4],
        )
        .expect("the initial thumbnail is exact"),
    );

    item.degrade_thumbnail(CaptureFailure::ProtectedContent);

    assert_eq!(item.title(), "Protected video");
    assert_eq!(item.application_icon().name(), "org.example.Player");
    assert!(item.thumbnail().is_none());
    assert_eq!(
        item.thumbnail_failure(),
        Some(CaptureFailure::ProtectedContent)
    );
}

#[test]
fn supported_shm_formats_are_normalized_for_overlay_rendering() {
    let frame = |format, packed: u32| {
        ThumbnailFrame::new(
            ShmFrameLayout {
                width: 1,
                height: 1,
                stride: 4,
                byte_len: 4,
                format,
            },
            packed.to_ne_bytes().to_vec(),
        )
        .expect("one exact pixel is valid")
    };

    assert_eq!(
        frame(ShmFormat::Argb8888, 0x7F11_2233).argb_pixel(0, 0),
        Some(0x7F11_2233)
    );
    assert_eq!(
        frame(ShmFormat::Xrgb8888, 0x0011_2233).argb_pixel(0, 0),
        Some(0xFF11_2233)
    );
    assert_eq!(
        frame(ShmFormat::Abgr8888, 0x7F33_2211).argb_pixel(0, 0),
        Some(0x7F11_2233)
    );
    assert_eq!(
        frame(ShmFormat::Xbgr8888, 0x0033_2211).argb_pixel(0, 0),
        Some(0xFF11_2233)
    );
}

#[test]
fn invalid_or_unsupported_constraints_degrade_without_requesting_a_buffer() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    captures.set_visible([window("invalid"), window("unsupported")]);

    assert_eq!(
        captures.initialized(
            &window("invalid"),
            &ShmConstraints {
                width: 1,
                height: u32::MAX,
                formats: vec![ShmFormat::Argb8888],
            },
        ),
        vec![CaptureEffect::DegradeThumbnail {
            window: window("invalid"),
            reason: CaptureFailure::InvalidDimensions,
        }]
    );
    assert_eq!(
        captures.initialized(
            &window("unsupported"),
            &ShmConstraints {
                width: 800,
                height: 600,
                formats: Vec::new(),
            },
        ),
        vec![CaptureEffect::DegradeThumbnail {
            window: window("unsupported"),
            reason: CaptureFailure::UnsupportedFormat,
        }]
    );
}

#[test]
fn compositor_transform_orients_content_before_fitting_it() {
    let frame = ThumbnailFrame::with_transform(
        ShmFrameLayout {
            width: 2,
            height: 1,
            stride: 8,
            byte_len: 8,
            format: ShmFormat::Xrgb8888,
        },
        [0x0011_2233_u32.to_ne_bytes(), 0x0044_5566_u32.to_ne_bytes()].concat(),
        BufferTransform::Rotate90,
    )
    .expect("the transformed frame is exact");

    assert_eq!(frame.presentation_size(), (1, 2));
    assert_eq!(frame.fitted_size(100, 100), (50, 100));
    assert_eq!(frame.argb_pixel(0, 0), Some(0xFF44_5566));
    assert_eq!(frame.argb_pixel(0, 1), Some(0xFF11_2233));
}

#[test]
fn every_compositor_transform_uses_one_consistent_geometry() {
    let pixels = (1_u32..=6).flat_map(u32::to_ne_bytes).collect::<Vec<_>>();
    let cases = [
        (BufferTransform::Normal, (3, 2), vec![1, 2, 3, 4, 5, 6]),
        (BufferTransform::Rotate90, (2, 3), vec![3, 6, 2, 5, 1, 4]),
        (BufferTransform::Rotate180, (3, 2), vec![6, 5, 4, 3, 2, 1]),
        (BufferTransform::Rotate270, (2, 3), vec![4, 1, 5, 2, 6, 3]),
        (BufferTransform::Flipped, (3, 2), vec![3, 2, 1, 6, 5, 4]),
        (BufferTransform::Flipped90, (2, 3), vec![1, 4, 2, 5, 3, 6]),
        (BufferTransform::Flipped180, (3, 2), vec![4, 5, 6, 1, 2, 3]),
        (BufferTransform::Flipped270, (2, 3), vec![6, 3, 5, 2, 4, 1]),
    ];

    for (transform, expected_size, expected_pixels) in cases {
        let frame = ThumbnailFrame::with_transform(
            ShmFrameLayout {
                width: 3,
                height: 2,
                stride: 12,
                byte_len: pixels.len(),
                format: ShmFormat::Argb8888,
            },
            pixels.clone(),
            transform,
        )
        .expect("the transformed frame is exact");

        assert_eq!(frame.presentation_size(), expected_size);
        let mut actual_pixels = Vec::new();
        for y in 0..expected_size.1 {
            for x in 0..expected_size.0 {
                actual_pixels.push(
                    frame
                        .argb_pixel(x, y)
                        .expect("the presentation coordinate maps into the frame"),
                );
            }
        }
        assert_eq!(actual_pixels, expected_pixels, "{transform:?}");
    }
}
