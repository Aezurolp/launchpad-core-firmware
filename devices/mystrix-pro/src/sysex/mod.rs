// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

pub mod device_inquiry;

use firmware_core::app::AppId;
use firmware_core::sys::led;
use firmware_core::sys::midi::MidiPort;
use firmware_core::sys::sysex::{DefaultSysExHandler, SysExHandler};

pub struct Handler;

impl SysExHandler for Handler {
    fn execute(app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if device_inquiry::Handler::execute(app, port, data) {
            return true;
        }
        if handle_led(data) {
            return true;
        }
        DefaultSysExHandler::execute(app, port, data)
    }
}

fn handle_led(data: &[u8]) -> bool {
    if data.len() < 8
        || data[0] != 0xf0
        || data.last() != Some(&0xf7)
        || !matches!(data, [0xf0, 0x00, 0x20, 0x29, 0x02, 0x10, ..])
    {
        return false;
    }
    match data[6] {
        0x0a => {
            for pair in data[7..data.len() - 1].chunks_exact(2) {
                if pair[0] <= 99 {
                    led::novation(pair[0], pair[1]);
                }
            }
        }
        0x0b => {
            for rgb in data[7..data.len() - 1].chunks_exact(4) {
                if rgb[0] <= 99 {
                    led::set_rgb(rgb[0], rgb[1] & 0x3f, rgb[2] & 0x3f, rgb[3] & 0x3f);
                }
            }
        }
        0x0c => handle_palette_column(data),
        0x0d => handle_palette_row(data),
        0x0f if data.len() >= 11 => {
            let grid_only = data[7] == 1;
            let (row_start, row_end, col_start, col_end) = if grid_only {
                (1, 8, 1, 8)
            } else {
                (0, 9, 0, 9)
            };
            let mut cursor = 8;
            for row in row_start..=row_end {
                for col in col_start..=col_end {
                    if cursor + 2 >= data.len() - 1 {
                        return true;
                    }
                    let index = row * 10 + col;
                    if !matches!(index, 0 | 9 | 90 | 99) {
                        led::set_rgb(
                            index,
                            data[cursor] & 0x3f,
                            data[cursor + 1] & 0x3f,
                            data[cursor + 2] & 0x3f,
                        );
                    }
                    cursor += 3;
                }
            }
        }
        _ => {}
    }
    true
}

fn handle_palette_column(data: &[u8]) {
    if data.len() < 9 || data[7] > 9 {
        return;
    }
    for (row, color) in data[8..data.len() - 1].iter().copied().enumerate().take(10) {
        if let Some(index) = map_grid(row as u8, data[7]) {
            led::novation(index, color);
        }
    }
}

fn handle_palette_row(data: &[u8]) {
    if data.len() < 9 || data[7] > 9 {
        return;
    }
    for (column, color) in data[8..data.len() - 1].iter().copied().enumerate().take(10) {
        if let Some(index) = map_grid(data[7], column as u8) {
            led::novation(index, color);
        }
    }
}

fn map_grid(row: u8, column: u8) -> Option<u8> {
    if row > 9 || column > 9 || matches!((row, column), (0 | 9, 0 | 9)) {
        None
    } else {
        Some(row * 10 + column)
    }
}
