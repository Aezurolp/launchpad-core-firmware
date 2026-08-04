// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Aezurolp

use core::cell::UnsafeCell;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Rotation {
    Default,
    Left,
    Right,
    UpsideDown,
}

struct RotationSlot {
    inner: UnsafeCell<Rotation>,
}

unsafe impl Sync for RotationSlot {}

static ROTATION: RotationSlot = RotationSlot {
    inner: UnsafeCell::new(Rotation::Default),
};

// Current rotation. Not persisted to flash.
pub fn get() -> Rotation {
    unsafe { *ROTATION.inner.get() }
}

pub fn set(rotation: Rotation) {
    unsafe {
        *ROTATION.inner.get() = rotation;
    }
}

// Physical to logical index.
pub fn to_canonical(raw_index: u8) -> u8 {
    apply(raw_index, get(), false, false)
}

// Logical to physical index.
pub fn to_raw(canonical_index: u8) -> u8 {
    apply(canonical_index, get(), true, false)
}

// Physical -> logical index, grid only.
pub fn to_canonical_grid_only(raw_index: u8) -> u8 {
    apply(raw_index, get(), false, true)
}

// Logical -> physical index, grid only.
pub fn to_raw_grid_only(canonical_index: u8) -> u8 {
    apply(canonical_index, get(), true, true)
}

fn apply(index: u8, rotation: Rotation, inverse: bool, grid_only: bool) -> u8 {
    if index == 99 {
        return 99;
    }

    let Some((row, col)) = decode(index) else {
        return index;
    };

    if grid_only && !(is_grid_axis(row) && is_grid_axis(col)) {
        return index;
    }

    let (row, col) = rotate_coords(row, col, rotation, inverse);
    encode(row, col)
}

fn is_grid_axis(value: u8) -> bool {
    (1..=8).contains(&value)
}

// Swap direction when applying the inverse.
fn rotate_coords(row: u8, col: u8, rotation: Rotation, inverse: bool) -> (u8, u8) {
    let effective = match (rotation, inverse) {
        (Rotation::Default, _) => Rotation::Default,
        (Rotation::Left, false) | (Rotation::Right, true) => Rotation::Left,
        (Rotation::Right, false) | (Rotation::Left, true) => Rotation::Right,
        (Rotation::UpsideDown, _) => Rotation::UpsideDown
    };

    match effective {
        Rotation::Default => (row, col),
        Rotation::Left => (col, 9 - row),
        Rotation::Right => (9 - col, row),
        Rotation::UpsideDown => (9 - row, 9 - col),
    }
}

// Index -> (row, col).
fn decode(index: u8) -> Option<(u8, u8)> {
    match index {
        1..=8 => Some((0, index)),
        91..=98 => Some((9, index - 90)),
        10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 => Some((index / 10, 0)),
        19 | 29 | 39 | 49 | 59 | 69 | 79 | 89 => Some((index / 10, 9)),
        11..=18 | 21..=28 | 31..=38 | 41..=48 | 51..=58 | 61..=68 | 71..=78 | 81..=88 => {
            Some((index / 10, index % 10))
        }
        _ => None,
    }
}

/// Inverse of `decode`.
fn encode(row: u8, col: u8) -> u8 {
    match (row, col) {
        (0, c) => c,
        (9, c) => 90 + c,
        (r, 0) => r * 10,
        (r, 9) => r * 10 + 9,
        (r, c) => r * 10 + c,
    }
}
