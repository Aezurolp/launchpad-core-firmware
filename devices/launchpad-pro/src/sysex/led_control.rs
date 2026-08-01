// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::sys::sysex::led_control;

const PRO_DEVICE_ID: u8 = 0x10;
const MAX_LED_INDEX: u8 = 99;

pub fn handle(data: &[u8]) -> bool {
    led_control::handle_legacy(data, PRO_DEVICE_ID, map_grid, MAX_LED_INDEX)
}

fn map_grid(row: u8, col: u8) -> Option<u8> {
    if row > 9 || col > 9 {
        return None;
    }
    if (row == 0 || row == 9) && (col == 0 || col == 9) {
        return None;
    }

    Some(row * 10 + col)
}
