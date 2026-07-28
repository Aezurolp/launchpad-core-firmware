// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister
// Copyright (C) 2026 ZephyrCodesStuff

use crate::extflash::ExtFlash;
use crate::grid::Grid;
use crate::usb;
use firmware_core::driver::Driver;
use firmware_core::sys::midi::MidiPort;

use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};

pub struct RuntimeDriver {
    grid: *mut Grid,
    flash: ExtFlash<'static>,
}

impl RuntimeDriver {
    pub fn new(grid: &mut Grid, flash: ExtFlash<'static>) -> Self {
        Self { grid, flash }
    }

    fn grid(&mut self) -> &mut Grid {
        unsafe { &mut *self.grid }
    }
}

impl Driver for RuntimeDriver {
    fn set_rgb_led(&mut self, index: u8, r: u8, g: u8, b: u8) {
        self.grid().set_led_rgb(index, r, g, b);
    }

    fn set_led(&mut self, index: u8, color: u32) {
        self.grid().set_led(index, color);
    }

    fn fill(&mut self, color: u32) {
        self.grid().fill(color);
    }

    fn brightness(&mut self) -> u8 {
        self.grid().brightness()
    }

    fn set_brightness(&mut self, brightness: u8) {
        self.grid().set_brightness(brightness);
    }

    fn send_midi(&mut self, port: MidiPort, data: &[u8]) {
        let _ = usb::enqueue_tx_message(port.as_cable(), data);
    }

    fn flash_size(&mut self) -> u32 {
        self.flash.capacity() as u32
    }

    fn read_flash(&mut self, offset: u32, data: &mut [u8]) {
        let _ = self.flash.read(offset, data);
    }

    fn write_flash(&mut self, offset: u32, data: &[u8]) {
        let _ = self.flash.write(offset, data);
    }

    fn device_id(&self) -> u8 {
        3
    }
}
