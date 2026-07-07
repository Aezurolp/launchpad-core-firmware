// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::app::{AftertouchEvent, App, MidiEvent, MidiPort, SurfaceEvent};
use firmware_core::driver;

pub struct LiveApp;

impl LiveApp {
    pub const fn new() -> Self {
        Self
    }
}

impl App for LiveApp {
    fn on_enter(&mut self) {
        // Clear all LEDs on enter
        driver::fill(0);
    }

    fn on_exit(&mut self) {}

    fn on_surface(&mut self, event: SurfaceEvent) {
        if event.pressed {
            // Light up green based on velocity
            let brightness = event.value.max(10) / 2;
            driver::set_rgb_led(event.index, 0, brightness, 0);

            // Send MIDI Note On on Channel 1
            driver::send_midi(MidiPort::Daw, &[0x90, event.index, event.value]);
        } else {
            // Turn off LED
            driver::set_rgb_led(event.index, 0, 0, 0);

            // Send MIDI Note Off
            driver::send_midi(MidiPort::Daw, &[0x80, event.index, 0]);
        }
    }

    fn on_midi(&mut self, _event: MidiEvent) {}

    fn on_aftertouch(&mut self, event: AftertouchEvent) {
        // Send Polyphonic Aftertouch
        driver::send_midi(MidiPort::Daw, &[0xa0, event.index, event.value]);
    }

    fn on_tick(&mut self) {}
}

pub const fn new() -> LiveApp {
    LiveApp::new()
}
