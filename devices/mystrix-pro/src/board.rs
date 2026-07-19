// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

//! Mystrix Pro hardware configuration, independently reimplemented from the
//! MIT-licensed MatrixOS Mystrix1 device definitions.

pub const FN_PIN: u8 = 16;
pub const LED_PIN: u8 = 38;
pub const USB_DM_PIN: u8 = 19;
pub const USB_DP_PIN: u8 = 20;

pub const KEYPAD_WRITE_PINS: [u8; 8] = [21, 17, 1, 6, 12, 13, 14, 15];
pub const KEYPAD_READ_PINS: [u8; 8] = [2, 3, 4, 5, 7, 8, 9, 10];

pub const GRID_LED_COUNT: usize = 64;
pub const UNDERGLOW_LED_COUNT: usize = 32;
pub const LED_COUNT: usize = GRID_LED_COUNT + UNDERGLOW_LED_COUNT;
pub const TOUCH_SEGMENT_COUNT: usize = 16;

const TOUCH_MAP_STANDARD: [u8; TOUCH_SEGMENT_COUNT] =
    [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 1, 2, 3];
const TOUCH_MAP_REVC: [u8; TOUCH_SEGMENT_COUNT] =
    [4, 5, 6, 7, 15, 14, 13, 12, 11, 10, 9, 8, 0, 1, 2, 3];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Revision {
    V100,
    V110,
    RevC,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardConfig {
    pub revision: Revision,
    pub touch_data_pin: u8,
    pub touch_clock_pin: u8,
    pub touch_map: [u8; TOUCH_SEGMENT_COUNT],
}

impl BoardConfig {
    pub const fn for_revision(revision: Revision) -> Self {
        match revision {
            Revision::V100 => Self {
                revision,
                touch_data_pin: 33,
                touch_clock_pin: 34,
                touch_map: TOUCH_MAP_STANDARD,
            },
            Revision::V110 => Self {
                revision,
                touch_data_pin: 47,
                touch_clock_pin: 33,
                touch_map: TOUCH_MAP_STANDARD,
            },
            Revision::RevC => Self {
                revision,
                touch_data_pin: 47,
                touch_clock_pin: 33,
                touch_map: TOUCH_MAP_REVC,
            },
        }
    }

    pub fn from_efuse_user_data(data: &[u8]) -> Self {
        let revision = data.get(4..8).unwrap_or_default();
        let revision = match revision {
            b"V100" => Revision::V100,
            b"V110" => Revision::V110,
            b"REVC" => Revision::RevC,
            _ => Revision::V110,
        };
        Self::for_revision(revision)
    }
}
