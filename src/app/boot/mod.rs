// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use crate::app::{AftertouchEvent, App, AppId, MidiEvent, SurfaceEvent};
use crate::sys::led;
use crate::utils::layout::dr_to_xy;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BootFrame {
    pub tick: u16,
    pub count: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BootChange {
    pub led: u8,
    pub velocity: u8,
}

pub struct BootAnimation {
    pub frames: &'static [BootFrame],
    pub changes: &'static [BootChange],
    pub end_tick: u16,
}

#[allow(dead_code)]
#[repr(C, align(4))]
struct AlignedBytes<const N: usize>([u8; N]);

#[cfg(feature = "launchpad-mini-mk3")]
static BOOT_DATA: AlignedBytes<5224> =
    AlignedBytes(*include_bytes!("../../../animations/launchpad-mini-mk3.bin"));

#[cfg(feature = "launchpad-mk2")]
static BOOT_DATA: AlignedBytes<5388> =
    AlignedBytes(*include_bytes!("../../../animations/launchpad-mk2.bin"));

#[cfg(feature = "launchpad-pro")]
static BOOT_DATA: AlignedBytes<5064> =
    AlignedBytes(*include_bytes!("../../../animations/launchpad-pro.bin"));

#[cfg(feature = "launchpad-pro-mk3")]
static BOOT_DATA: AlignedBytes<6472> =
    AlignedBytes(*include_bytes!("../../../animations/launchpad-pro-mk3.bin"));

// Animation for X and fallback for mini mk1 and s
#[cfg(not(any(
    feature = "launchpad-mini-mk3",
    feature = "launchpad-mk2",
    feature = "launchpad-pro",
    feature = "launchpad-pro-mk3"
)))]
static BOOT_DATA: AlignedBytes<5332> =
    AlignedBytes(*include_bytes!("../../../animations/launchpad-x.bin"));

static BOOT_ANIMATION: BootAnimation = {
    let bytes = &BOOT_DATA.0;

    let end_tick = u16::from_le_bytes([bytes[0], bytes[1]]);
    let num_frames = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    let num_changes = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;

    let frames_ptr = bytes.as_ptr().wrapping_add(8) as *const BootFrame;
    let changes_ptr = bytes
        .as_ptr()
        .wrapping_add(8 + num_frames * core::mem::size_of::<BootFrame>())
        as *const BootChange;

    let frames = unsafe { core::slice::from_raw_parts(frames_ptr, num_frames) };
    let changes = unsafe { core::slice::from_raw_parts(changes_ptr, num_changes) };

    BootAnimation {
        frames,
        changes,
        end_tick,
    }
};

pub struct BootApp {
    animation: &'static BootAnimation,
    tick: u16,
    frame_index: usize,
    change_index: usize,
    requested_switch: Option<AppId>,
}

pub type BootAnimationApp = BootApp;

impl BootApp {
    pub const fn new() -> Self {
        Self {
            animation: &BOOT_ANIMATION,
            tick: 0,
            frame_index: 0,
            change_index: 0,
            requested_switch: None,
        }
    }
}

impl App for BootApp {
    fn on_enter(&mut self) {
        self.tick = 0;
        self.frame_index = 0;
        self.change_index = 0;
        self.requested_switch = None;
        led::clear();
    }

    fn on_exit(&mut self) {}

    fn on_surface(&mut self, _event: SurfaceEvent) {}

    fn on_midi(&mut self, _event: MidiEvent) {}

    fn on_aftertouch(&mut self, _event: AftertouchEvent) {}

    fn on_tick(&mut self) {
        while self.frame_index < self.animation.frames.len()
            && self.animation.frames[self.frame_index].tick == self.tick
        {
            let count = self.animation.frames[self.frame_index].count;

            for _ in 0..count {
                if self.change_index >= self.animation.changes.len() {
                    break;
                }

                let change = self.animation.changes[self.change_index];
                self.change_index += 1;
                led::novation(dr_to_xy(change.led), change.velocity);
            }

            self.frame_index += 1;
        }

        if self.tick >= self.animation.end_tick {
            self.requested_switch = Some(AppId::Performance);
            return;
        }

        self.tick = self.tick.saturating_add(1);
    }

    fn take_requested_app_switch(&mut self) -> Option<AppId> {
        self.requested_switch.take()
    }
}
