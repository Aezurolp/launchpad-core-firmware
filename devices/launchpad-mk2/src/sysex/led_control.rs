// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::sys::sysex::led_control;

const MK2_DEVICE_ID: u8 = 0x18;
const MAX_LED_INDEX: u8 = 111;

pub fn handle(data: &[u8]) -> bool {
    led_control::handle_legacy(data, MK2_DEVICE_ID, map_grid, MAX_LED_INDEX)
}

fn map_grid(row: u8, col: u8) -> Option<u8> {
    if row > 8 || col > 8 {
        return None;
    }

    if row < 8 && col < 8 {
        Some((row + 1) * 10 + (col + 1))
    } else if row == 8 && col < 8 {
        Some(104 + col)
    } else if col == 8 && row < 8 {
        Some((row + 1) * 10 + 9)
    } else {
        None
    }
}
