// SPDX-License-Identifier: GPL-3.0-only

use std::time::Duration;

use cosmic_window_switcher::{
    BufferTransform, CaptureBackend, CaptureEffect, CaptureFailure, CaptureOpportunity,
    CaptureSessionModel, DmaBufCompatibility, DmaBufContractStatus, DmaBufFallbackReason,
    FrameDamage, RefreshCeiling, ShmConstraints, ShmFormat, ShmFrameLayout, SwitcherItem,
    ThumbnailFrame, WindowId,
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

fn request_every_due_frame(captures: &mut CaptureSessionModel, now: Duration) {
    while captures
        .next_request_at()
        .is_some_and(|deadline| deadline <= now)
    {
        let effects = captures.refresh_due(now, CaptureOpportunity::InputDrained);
        assert!(
            !effects.is_empty(),
            "due capture work must make bounded progress"
        );
    }
}

#[test]
fn dma_buf_is_selected_only_for_a_complete_compositor_to_renderer_contract() {
    let selection = DmaBufCompatibility::complete().select_backend();

    assert_eq!(selection.backend(), CaptureBackend::DmaBuf);
    assert_eq!(selection.fallback_reason(), None);
}

#[test]
fn every_incompatible_dma_buf_stage_falls_back_to_shared_memory() {
    let cases = [
        (
            DmaBufCompatibility {
                device: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::IncompatibleDevice,
        ),
        (
            DmaBufCompatibility {
                format: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::UnsupportedFormat,
        ),
        (
            DmaBufCompatibility {
                modifier: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::UnsupportedModifier,
        ),
        (
            DmaBufCompatibility {
                allocation: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::AllocationFailed,
        ),
        (
            DmaBufCompatibility {
                synchronization: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::SynchronizationUnavailable,
        ),
        (
            DmaBufCompatibility {
                import: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::ImportFailed,
        ),
        (
            DmaBufCompatibility {
                release: DmaBufContractStatus::Incompatible,
                ..DmaBufCompatibility::complete()
            },
            DmaBufFallbackReason::ReleaseUnavailable,
        ),
    ];

    for (compatibility, reason) in cases {
        let selection = compatibility.select_backend();

        assert_eq!(selection.backend(), CaptureBackend::SharedMemory);
        assert_eq!(selection.fallback_reason(), Some(reason));
    }
}

#[test]
fn visible_windows_negotiate_exact_shm_frames_with_one_request_outstanding() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    captures.set_selected(Some(window("selected")));

    assert_eq!(
        captures.set_visible([window("selected"), window("other")]),
        vec![
            CaptureEffect::CreateStream(window("selected")),
            CaptureEffect::CreateStream(window("other")),
        ]
    );
    assert!(
        captures
            .initialized(&window("selected"), &constraints())
            .is_empty()
    );
    assert!(
        captures
            .initialized(&window("other"), &constraints())
            .is_empty()
    );
    assert_eq!(
        captures.refresh_due(Duration::ZERO, CaptureOpportunity::InputDrained),
        vec![
            CaptureEffect::RequestFrame {
                window: window("selected"),
                layout: ShmFrameLayout {
                    width: 1_920,
                    height: 1_080,
                    stride: 7_680,
                    byte_len: 8_294_400,
                    format: ShmFormat::Argb8888,
                },
            },
            CaptureEffect::RequestFrame {
                window: window("other"),
                layout: ShmFrameLayout {
                    width: 1_920,
                    height: 1_080,
                    stride: 7_680,
                    byte_len: 8_294_400,
                    format: ShmFormat::Argb8888,
                },
            },
        ]
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
    request_every_due_frame(&mut captures, Duration::ZERO);

    assert_eq!(
        captures.frame_ready(
            &window("selected"),
            Duration::ZERO,
            &[FrameDamage::new(0, 0, 1_920, 1_080)],
        ),
        vec![CaptureEffect::PresentThumbnail(window("selected"))]
    );
    assert!(
        captures
            .refresh_due(Duration::from_millis(32), CaptureOpportunity::InputDrained,)
            .is_empty()
    );
    assert_eq!(
        captures.refresh_due(Duration::from_millis(34), CaptureOpportunity::InputDrained,),
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
fn match_display_uses_the_session_displays_current_refresh_rate() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::MatchDisplay);
    captures.set_display_refresh_rate(120);
    captures.set_visible([window("selected")]);
    captures.initialized(&window("selected"), &constraints());
    request_every_due_frame(&mut captures, Duration::ZERO);
    captures.frame_ready(
        &window("selected"),
        Duration::ZERO,
        &[FrameDamage::new(0, 0, 1_920, 1_080)],
    );

    assert!(
        captures
            .refresh_due(Duration::from_millis(8), CaptureOpportunity::InputDrained,)
            .is_empty()
    );
    assert_eq!(
        captures.refresh_due(Duration::from_millis(9), CaptureOpportunity::InputDrained,),
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
}

#[test]
fn input_runs_before_selected_and_bounded_round_robin_capture_work() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    let windows = [
        window("background-a"),
        window("selected"),
        window("background-b"),
        window("background-c"),
    ];
    captures.set_visible(windows.clone());
    captures.set_selected(Some(window("selected")));
    for window in &windows {
        captures.initialized(window, &constraints());
    }
    request_every_due_frame(&mut captures, Duration::ZERO);
    for window in &windows {
        captures.frame_ready(
            window,
            Duration::ZERO,
            &[FrameDamage::new(0, 0, 1_920, 1_080)],
        );
    }

    assert!(
        captures
            .refresh_due(Duration::from_millis(34), CaptureOpportunity::InputPending,)
            .is_empty()
    );
    assert_eq!(
        captures.refresh_due(Duration::from_millis(34), CaptureOpportunity::InputDrained,),
        vec![
            CaptureEffect::RequestFrame {
                window: window("selected"),
                layout: constraints()
                    .negotiate()
                    .expect("SHM constraints are valid"),
            },
            CaptureEffect::RequestFrame {
                window: window("background-a"),
                layout: constraints()
                    .negotiate()
                    .expect("SHM constraints are valid"),
            },
        ]
    );
    assert_eq!(
        captures.refresh_due(Duration::from_millis(34), CaptureOpportunity::InputDrained,),
        vec![CaptureEffect::RequestFrame {
            window: window("background-b"),
            layout: constraints()
                .negotiate()
                .expect("SHM constraints are valid"),
        }]
    );
    assert_eq!(
        captures.refresh_due(Duration::from_millis(34), CaptureOpportunity::InputDrained,),
        vec![CaptureEffect::RequestFrame {
            window: window("background-c"),
            layout: constraints()
                .negotiate()
                .expect("SHM constraints are valid"),
        }]
    );
}

#[test]
fn ten_window_overload_remains_fair_and_recovers_without_duplicate_requests() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    let windows = (0..10)
        .map(|index| window(&format!("window-{index}")))
        .collect::<Vec<_>>();
    captures.set_visible(windows.clone());
    captures.set_selected(Some(windows[5].clone()));
    for window in &windows {
        captures.initialized(window, &constraints());
    }
    request_every_due_frame(&mut captures, Duration::ZERO);
    for window in &windows {
        captures.frame_ready(
            window,
            Duration::ZERO,
            &[FrameDamage::new(0, 0, 1_920, 1_080)],
        );
    }

    let mut requested = Vec::new();
    while captures.next_request_at().is_some() {
        let effects =
            captures.refresh_due(Duration::from_millis(34), CaptureOpportunity::InputDrained);
        assert!(effects.len() <= 2);
        requested.extend(effects.into_iter().map(|effect| match effect {
            CaptureEffect::RequestFrame { window, .. } => window,
            other => panic!("unexpected scheduling effect: {other:?}"),
        }));
    }

    assert_eq!(requested.first(), Some(&windows[5]));
    requested.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut expected = windows;
    expected.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    assert_eq!(requested, expected);
}

#[test]
fn idle_service_has_no_capture_deadline_or_capture_work() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);

    assert_eq!(captures.active_stream_count(), 0);
    assert_eq!(captures.next_request_at(), None);
    assert!(
        captures
            .refresh_due(Duration::MAX, CaptureOpportunity::InputDrained)
            .is_empty()
    );
}

#[test]
fn unsolicited_frame_completion_does_not_create_capture_work() {
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    captures.set_visible([window("visible")]);
    captures.initialized(&window("visible"), &constraints());

    assert!(
        captures
            .frame_ready(
                &window("visible"),
                Duration::ZERO,
                &[FrameDamage::new(0, 0, 1_920, 1_080)],
            )
            .is_empty()
    );
    assert_eq!(captures.next_request_at(), Some(Duration::ZERO));
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

#[test]
fn malformed_zero_sized_frame_fits_to_nothing_without_panicking() {
    let frame = ThumbnailFrame::new(
        ShmFrameLayout {
            width: 0,
            height: 0,
            stride: 0,
            byte_len: 0,
            format: ShmFormat::Argb8888,
        },
        Vec::new(),
    )
    .expect("the direct frame constructor accepts an exact empty allocation");

    assert_eq!(frame.fitted_size(100, 100), (0, 0));
    assert_eq!(frame.argb_pixel(0, 0), None);
}
