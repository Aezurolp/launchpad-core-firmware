// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::sys::led;

pub fn handle(data: &[u8]) -> bool {
    if data.len() < 8 || data[0] != 0xf0 || data.last() != Some(&0xf7) {
        return false;
    }
    if !matches!(data, [0xf0, 0x00, 0x20, 0x29, 0x02, 0x10, ..]) {
        return false;
    }

    match data[6] {
        0x0a => handle_palette_pairs(data),
        0x0b => handle_rgb_groups(data),
        0x0c => handle_palette_column(data),
        0x0d => handle_palette_row(data),
        0x0f => handle_rgb_grid(data),
        _ => true,
    }
}

fn handle_palette_pairs(data: &[u8]) -> bool {
    let mut index = 7;
    while index + 1 < data.len() - 1 {
        let led_index = data[index];
        let velocity = data[index + 1];
        index += 2;

        if led_index <= 99 {
            led::novation(led_index, velocity);
        }
    }
    true
}

fn handle_rgb_groups(data: &[u8]) -> bool {
    let mut index = 7;
    while index + 3 < data.len() - 1 {
        let led_index = data[index];
        let r = data[index + 1] & 0x3f;
        let g = data[index + 2] & 0x3f;
        let b = data[index + 3] & 0x3f;
        index += 4;

        if led_index <= 99 {
            led::set_rgb(led_index, r, g, b);
        }
    }
    true
}

fn handle_palette_column(data: &[u8]) -> bool {
    if data.len() < 9 {
        return true;
    }

    let col = data[7];
    if col > 9 {
        return true;
    }

    let mut index = 8;
    let mut row = 0;
    while index < data.len() - 1 && row <= 9 {
        if let Some(led_index) = map_grid(row, col) {
            led::novation(led_index, data[index]);
        }
        index += 1;
        row += 1;
    }
    true
}

fn handle_palette_row(data: &[u8]) -> bool {
    if data.len() < 9 {
        return true;
    }

    let row = data[7];
    if row > 9 {
        return true;
    }

    let mut index = 8;
    let mut col = 0;
    while index < data.len() - 1 && col <= 9 {
        if let Some(led_index) = map_grid(row, col) {
            led::novation(led_index, data[index]);
        }
        index += 1;
        col += 1;
    }
    true
}

fn handle_rgb_grid(data: &[u8]) -> bool {
    if data.len() < 11 {
        return true;
    }

    let mut index = 8;
    match data[7] {
        0 => {
            for row in 0..=9 {
                for col in 0..=9 {
                    if index + 2 >= data.len() - 1 {
                        return true;
                    }
                    let r = data[index] & 0x3f;
                    let g = data[index + 1] & 0x3f;
                    let b = data[index + 2] & 0x3f;
                    index += 3;

                    if let Some(led_index) = map_grid(row, col) {
                        led::set_rgb(led_index, r, g, b);
                    }
                }
            }
        }
        1 => {
            for row in 1..=8 {
                for col in 1..=8 {
                    if index + 2 >= data.len() - 1 {
                        return true;
                    }
                    let r = data[index] & 0x3f;
                    let g = data[index + 1] & 0x3f;
                    let b = data[index + 2] & 0x3f;
                    index += 3;

                    if let Some(led_index) = map_grid(row, col) {
                        led::set_rgb(led_index, r, g, b);
                    }
                }
            }
        }
        _ => {}
    }

    true
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
