// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

pub trait LedTarget {
    fn set_palette(&mut self, index: u8, velocity: u8);
    fn set_rgb(&mut self, index: u8, r: u8, g: u8, b: u8);
}

pub fn handle_modern(data: &[u8], device_id: u8, target: &mut impl LedTarget) -> bool {
    if data.len() < 8 || data[0] != 0xf0 || data.last() != Some(&0xf7) {
        return false;
    }

    if !matches!(
        data,
        [0xf0, 0x00, 0x20, 0x29, 0x02, id, 0x03, ..] if *id == device_id
    ) {
        return false;
    }

    let mut index = 7;
    while index < data.len() - 1 {
        if index + 2 > data.len() - 1 {
            break;
        }

        let lighting_type = data[index];
        let led_index = data[index + 1];
        index += 2;

        match lighting_type {
            0 => {
                if index >= data.len() - 1 {
                    break;
                }
                target.set_palette(led_index, data[index]);
                index += 1;
            }

            1 => {
                if index + 2 > data.len() - 1 {
                    break;
                }
                index += 2;
            }

            2 => {
                if index + 1 > data.len() - 1 {
                    break;
                }
                index += 1;
            }

            3 => {
                if index + 3 > data.len() - 1 {
                    break;
                }
                let r = data[index] & 0x3f;
                let g = data[index + 1] & 0x3f;
                let b = data[index + 2] & 0x3f;
                index += 3;
                target.set_rgb(led_index, r, g, b);
            }

            _ => break,
        }
    }

    true
}

pub fn handle_legacy(
    data: &[u8],
    device_id: u8,
    map_grid: impl Fn(u8, u8) -> Option<u8>,
    max_led_index: u8,
) -> bool {
    if data.len() < 8 || data[0] != 0xf0 || data.last() != Some(&0xf7) {
        return false;
    }
    if !matches!(data, [0xf0, 0x00, 0x20, 0x29, 0x02, id, ..] if *id == device_id) {
        return false;
    }

    match data[6] {
        0x0a => {
            let mut index = 7;
            while index + 1 < data.len() - 1 {
                let led_index = data[index];
                let velocity = data[index + 1];
                index += 2;

                if led_index <= max_led_index {
                    crate::sys::led::novation(led_index, velocity);
                }
            }
            true
        }
        0x0b => {
            let mut index = 7;
            while index + 3 < data.len() - 1 {
                let led_index = data[index];
                let r = data[index + 1] & 0x3f;
                let g = data[index + 2] & 0x3f;
                let b = data[index + 3] & 0x3f;
                index += 4;

                if led_index <= max_led_index {
                    crate::sys::led::set_rgb(led_index, r, g, b);
                }
            }
            true
        }
        0x0c => {
            if data.len() < 9 {
                return true;
            }
            let col = data[7];
            let mut index = 8;
            let mut row = 0;
            while index < data.len() - 1 {
                if let Some(led_index) = map_grid(row, col) {
                    crate::sys::led::novation(led_index, data[index]);
                }
                index += 1;
                row += 1;
            }
            true
        }
        0x0d => {
            if data.len() < 9 {
                return true;
            }
            let row = data[7];
            let mut index = 8;
            let mut col = 0;
            while index < data.len() - 1 {
                if let Some(led_index) = map_grid(row, col) {
                    crate::sys::led::novation(led_index, data[index]);
                }
                index += 1;
                col += 1;
            }
            true
        }
        0x0e => {
            if data.len() >= 8 {
                let velocity = data[7];
                for led_index in 0..=max_led_index {
                    crate::sys::led::novation(led_index, velocity);
                }
            }
            true
        }
        0x0f => {
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
                                crate::sys::led::set_rgb(led_index, r, g, b);
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
                                crate::sys::led::set_rgb(led_index, r, g, b);
                            }
                        }
                    }
                }
                _ => {}
            }
            true
        }
        _ => true,
    }
}

