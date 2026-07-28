// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::app::{BootAnimation, BootAnimationApp, BootChange, BootFrame};

pub type BootApp = BootAnimationApp;

#[repr(C, align(4))]
struct AlignedBytes<const N: usize>([u8; N]);

struct BootHeader {
    end_tick: u16,
    frame_count: usize,
    change_count: usize,
}

const BOOT_DATA_LEN: usize = 6472;
const BOOT_BYTES: &[u8; BOOT_DATA_LEN] = include_bytes!("../../../animations/boot-pro-mk3.bin");

const fn boot_header(bytes: &[u8; BOOT_DATA_LEN]) -> BootHeader {
    let end_tick = u16::from_le_bytes([bytes[0], bytes[1]]);
    let frame_count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    let change_count = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let payload_len = frame_count * core::mem::size_of::<BootFrame>()
        + change_count * core::mem::size_of::<BootChange>();

    assert!(8 + payload_len == BOOT_DATA_LEN);

    BootHeader {
        end_tick,
        frame_count,
        change_count,
    }
}

const BOOT_HEADER: BootHeader = boot_header(BOOT_BYTES);
static BOOT_DATA: AlignedBytes<BOOT_DATA_LEN> = AlignedBytes(*BOOT_BYTES);

static BOOT_ANIMATION: BootAnimation = {
    let bytes = &BOOT_DATA.0;
    let frames_ptr = bytes.as_ptr().wrapping_add(8) as *const BootFrame;
    let changes_ptr = bytes
        .as_ptr()
        .wrapping_add(8 + BOOT_HEADER.frame_count * core::mem::size_of::<BootFrame>())
        as *const BootChange;

    let frames = unsafe { core::slice::from_raw_parts(frames_ptr, BOOT_HEADER.frame_count) };
    let changes = unsafe { core::slice::from_raw_parts(changes_ptr, BOOT_HEADER.change_count) };

    BootAnimation {
        frames,
        changes,
        end_tick: BOOT_HEADER.end_tick,
    }
};

pub const fn new() -> BootApp {
    BootAnimationApp::new(&BOOT_ANIMATION)
}
