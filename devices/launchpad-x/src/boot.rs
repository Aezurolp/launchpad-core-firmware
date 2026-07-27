// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister
// Copyright (C) 2026      ZephyrCodesStuff + Gemini 3.6 Flash

use firmware_core::app::{BootAnimation, BootAnimationApp, BootChange, BootFrame};

pub type BootApp = BootAnimationApp;

#[repr(C, align(4))]
struct AlignedBytes<const N: usize>([u8; N]);

static BOOT_DATA: AlignedBytes<5332> =
    AlignedBytes(*include_bytes!("../../../animations/boot.bin"));

static BOOT_ANIMATION: BootAnimation = {
    let bytes = &BOOT_DATA.0;

    let end_tick = u16::from_le_bytes([bytes[0], bytes[1]]);
    let num_frames = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    let num_changes = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;

    let frames_ptr = bytes.as_ptr().wrapping_add(8) as *const BootFrame;
    let changes_ptr = bytes.as_ptr().wrapping_add(8 + num_frames * 4) as *const BootChange;

    let frames = unsafe { core::slice::from_raw_parts(frames_ptr, num_frames) };
    let changes = unsafe { core::slice::from_raw_parts(changes_ptr, num_changes) };

    BootAnimation {
        frames,
        changes,
        end_tick,
    }
};

pub const fn new() -> BootApp {
    BootAnimationApp::new(&BOOT_ANIMATION)
}

