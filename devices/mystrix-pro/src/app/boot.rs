// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::app::{AftertouchEvent, App, AppId, MidiEvent, SurfaceEvent};
use firmware_core::driver;

pub struct BootApp {
    ticks: u16,
    done: bool,
}

impl BootApp {
    pub const fn new() -> Self {
        Self {
            ticks: 0,
            done: false,
        }
    }
}

impl App for BootApp {
    fn on_enter(&mut self) {
        self.ticks = 0;
        self.done = false;
        driver::fill(0);
        for row in 1..=8 {
            for col in 1..=8 {
                let index = row * 10 + col;
                driver::set_rgb_led(index, (col * 6) as u8, (row * 6) as u8, 24);
            }
        }
        for col in 1..=8 {
            driver::set_rgb_led(col, (col * 6) as u8, 6, 16);
            driver::set_rgb_led(90 + col, (col * 6) as u8, 54, 24);
        }
        for row in 1..=8 {
            driver::set_rgb_led(row * 10, 6, (row * 6) as u8, 16);
            driver::set_rgb_led(row * 10 + 9, 54, (row * 6) as u8, 24);
        }
    }
    fn on_exit(&mut self) {}
    fn on_surface(&mut self, _event: SurfaceEvent) {}
    fn on_midi(&mut self, _event: MidiEvent) {}
    fn on_aftertouch(&mut self, _event: AftertouchEvent) {}
    fn on_tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        if self.ticks >= 700 {
            self.done = true;
        }
    }
    fn take_requested_app_switch(&mut self) -> Option<AppId> {
        if self.done {
            self.done = false;
            Some(AppId::Performance)
        } else {
            None
        }
    }
}

pub const fn new() -> BootApp {
    BootApp::new()
}
