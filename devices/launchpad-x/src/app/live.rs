// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::app::{AftertouchEvent, App, MidiEvent, SurfaceEvent};

pub struct LiveApp;

impl LiveApp {
    pub const fn new() -> Self {
        Self
    }
}

impl App for LiveApp {
    fn on_enter(&mut self) {}

    fn on_exit(&mut self) {}

    fn on_surface(&mut self, _event: SurfaceEvent) {}

    fn on_midi(&mut self, _event: MidiEvent) {}

    fn on_aftertouch(&mut self, _event: AftertouchEvent) {}

    fn on_tick(&mut self) {}
}

pub const fn new() -> LiveApp {
    LiveApp::new()
}
