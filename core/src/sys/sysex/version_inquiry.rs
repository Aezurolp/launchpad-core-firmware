// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use crate::app::AppId;
use crate::sys::midi::MidiPort;

use super::SysExHandler;

pub struct Handler;

const fn parse_u8(s: &str) -> u8 {
    let bytes = s.as_bytes();
    let mut val = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b >= b'0' && b <= b'9' {
            val = val * 10 + (b - b'0');
        }
        i += 1;
    }
    val
}

impl SysExHandler for Handler {
    fn execute(_app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if data.starts_with(&[0xf0, 0x00, 0x20, 0x29, 0x02, 0x7f, 0x00]) {
            const MAJOR: u8 = parse_u8(env!("CARGO_PKG_VERSION_MAJOR"));
            const MINOR: u8 = parse_u8(env!("CARGO_PKG_VERSION_MINOR"));
            const PATCH: u8 = parse_u8(env!("CARGO_PKG_VERSION_PATCH"));

            let device_id = crate::driver::device_id();

            let resp = [
                0xf0,
                0x00,
                0x20,
                0x29,
                0x02,
                0x7f,
                0x01,
                device_id,
                MAJOR & 0x7f,
                MINOR & 0x7f,
                PATCH & 0x7f,
                0xf7,
            ];

            crate::driver::send_midi(port, &resp);
            true
        } else {
            false
        }
    }
}
