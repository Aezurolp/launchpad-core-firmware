// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use esp_hal::Blocking;
use esp_hal::gpio::Level;
use esp_hal::rmt::{Channel, PulseCode, Rmt, Tx, TxChannelConfig, TxChannelCreator};
use esp_hal::time::Rate;
use crate::board::LED_COUNT;
use crate::grid::PHYSICAL_TO_LOGICAL;

const PULSES_PER_LED: usize = 24;
const PULSE_COUNT: usize = LED_COUNT * PULSES_PER_LED + 1;
const BRIGHTNESS: [u8; 8] = [22, 37, 54, 74, 95, 118, 142, 169];

pub struct Leds {
    channel: Option<Channel<'static, Blocking, Tx>>,
    framebuffer: [[u8; 3]; 100],
    brightness: u8,
    pulses: [PulseCode; PULSE_COUNT],
}

impl Leds {
    pub fn new(
        rmt: esp_hal::peripherals::RMT<'static>,
        pin: esp_hal::peripherals::GPIO38<'static>,
    ) -> Self {
        let rmt = Rmt::new(rmt, Rate::from_mhz(10)).expect("RMT clock");
        let config = TxChannelConfig::default()
            .with_clk_divider(1)
            .with_idle_output(true)
            .with_idle_output_level(Level::Low)
            .with_memsize(4);
        let channel = rmt
            .channel0
            .configure_tx(&config)
            .expect("RMT channel")
            .with_pin(pin);
        Self {
            channel: Some(channel),
            framebuffer: [[0; 3]; 100],
            brightness: 7,
            pulses: [PulseCode::end_marker(); PULSE_COUNT],
        }
    }

    pub fn set_rgb6(&mut self, index: u8, r: u8, g: u8, b: u8) {
        if let Some(pixel) = self.framebuffer.get_mut(index as usize) {
            *pixel = [expand6(r), expand6(g), expand6(b)];
        }
    }

    pub fn set_rgb8(&mut self, index: u8, color: u32) {
        if let Some(pixel) = self.framebuffer.get_mut(index as usize) {
            *pixel = [
                ((color >> 16) & 0xff) as u8,
                ((color >> 8) & 0xff) as u8,
                (color & 0xff) as u8,
            ];
        }
    }

    pub fn fill(&mut self, color: u32) {
        let rgb = [
            ((color >> 16) & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            (color & 0xff) as u8,
        ];
        self.framebuffer.fill(rgb);
    }

    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    pub fn set_brightness(&mut self, brightness: u8) {
        self.brightness = brightness.min(7);
    }

    pub fn show(&mut self) {
        let scale = BRIGHTNESS[self.brightness as usize];
        let mut pulse_index = 0;
        for (physical, logical) in PHYSICAL_TO_LOGICAL.iter().copied().enumerate() {
            let pixel = self.framebuffer[logical as usize];
            let underglow_gain = if physical >= 64 { 4 } else { 1 };
            let bytes = [pixel[1], pixel[0], pixel[2]];
            for byte in bytes {
                let scaled = (((byte as u16 * scale as u16) >> 8) * underglow_gain).min(255) as u8;
                for bit in (0..8).rev() {
                    self.pulses[pulse_index] = if scaled & (1 << bit) == 0 {
                        PulseCode::new(Level::High, 3, Level::Low, 9)
                    } else {
                        PulseCode::new(Level::High, 6, Level::Low, 6)
                    };
                    pulse_index += 1;
                }
            }
        }
        self.pulses[pulse_index] = PulseCode::new(Level::Low, 1_000, Level::Low, 0);

        if let Some(channel) = self.channel.take() {
            self.channel = match channel.transmit(&self.pulses) {
                Ok(transaction) => match transaction.wait() {
                    Ok(channel) => Some(channel),
                    Err((_error, channel)) => Some(channel),
                },
                Err((_error, channel)) => Some(channel),
            };
        }
    }
}

fn expand6(value: u8) -> u8 {
    let value = value & 0x3f;
    (value << 2) | (value >> 4)
}
