// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use crate::leds::Leds;
use crate::storage::SettingsFlash;
use crate::grid::Grid;
use firmware_core::driver::Driver;
use firmware_core::sys::midi::MidiPort;

pub struct Hardware {
    pub leds: Leds,
    pub grid: Grid,
    pub flash: SettingsFlash,
}

pub struct RuntimeDriver {
    hardware: *mut Hardware,
}

impl RuntimeDriver {
    pub const fn new(hardware: *mut Hardware) -> Self {
        Self { hardware }
    }

    fn hardware(&mut self) -> &mut Hardware {
        unsafe { &mut *self.hardware }
    }
}

impl Driver for RuntimeDriver {
    fn set_rgb_led(&mut self, index: u8, r: u8, g: u8, b: u8) {
        self.hardware().leds.set_rgb6(index, r, g, b);
    }
    fn set_led(&mut self, index: u8, color: u32) {
        self.hardware().leds.set_rgb8(index, color);
    }
    fn fill(&mut self, color: u32) {
        self.hardware().leds.fill(color);
    }
    fn brightness(&mut self) -> u8 {
        self.hardware().leds.brightness()
    }
    fn set_brightness(&mut self, brightness: u8) {
        self.hardware().leds.set_brightness(brightness);
    }
    fn send_midi(&mut self, _port: MidiPort, data: &[u8]) {
        crate::usb::enqueue_tx_message(data);
    }
    fn flash_size(&mut self) -> u32 {
        self.hardware().flash.settings_size()
    }
    fn read_flash(&mut self, offset: u32, data: &mut [u8]) {
        self.hardware().flash.read(offset, data);
    }
    fn write_flash(&mut self, offset: u32, data: &[u8]) {
        self.hardware().flash.write(offset, data);
    }
    fn device_id(&self) -> u8 {
        81
    }
}
