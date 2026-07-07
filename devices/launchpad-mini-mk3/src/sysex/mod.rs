// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

pub mod device_inquiry;

use firmware_core::app::AppId;
use firmware_core::sys::led;
use firmware_core::sys::midi::MidiPort;
use firmware_core::sys::sysex::{DefaultSysExHandler, SysExHandler, led_control};

const LED_SYSEX_DEVICE_ID: u8 = 0x0d;

pub struct Handler;

impl SysExHandler for Handler {
    fn execute(app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if device_inquiry::Handler::execute(app, port, data) {
            return true;
        }

        if led_control::handle_modern(data, LED_SYSEX_DEVICE_ID, &mut LedTarget) {
            return true;
        }

        DefaultSysExHandler::execute(app, port, data)
    }
}

struct LedTarget;

impl led_control::LedTarget for LedTarget {
    fn set_palette(&mut self, index: u8, velocity: u8) {
        led::set_palette(index, velocity);
    }

    fn set_rgb(&mut self, index: u8, r: u8, g: u8, b: u8) {
        led::set_rgb(index, r, g, b);
    }
}
