// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use crate::app::{AftertouchEvent, App, AppId, MidiEvent, SurfaceEvent};
use crate::sys::led;

#[derive(Copy, Clone)]
pub struct BootFrame {
    pub tick: u16,
    pub count: u8,
}

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

pub struct BootAnimationApp {
    animation: &'static BootAnimation,
    tick: u16,
    frame_index: usize,
    change_index: usize,
    requested_switch: Option<AppId>,
}

impl BootAnimationApp {
    pub const fn new(animation: &'static BootAnimation) -> Self {
        Self {
            animation,
            tick: 0,
            frame_index: 0,
            change_index: 0,
            requested_switch: None,
        }
    }
}

impl App for BootAnimationApp {
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
                led::novation(change.led, change.velocity);
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
