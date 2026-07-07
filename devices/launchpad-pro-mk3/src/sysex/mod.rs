// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use firmware_core::app::AppId;
use firmware_core::sys::midi::MidiPort;
use firmware_core::sys::sysex::{DefaultSysExHandler, SysExHandler, fastled, led_control};
use firmware_core::{driver, sys::led};

use crate::runtime::{
    self, M0_ROM_STATUS_ARG, M0_ROM_STATUS_OK, M0_ROM_STATUS_RX, M0FirmwareStatus, M0ProbeResult,
};

const NOVATION_HEADER: [u8; 6] = [0xf0, 0x00, 0x20, 0x29, 0x02, 0x0e];
const LED_SYSEX_DEVICE_ID: u8 = 0x0e;
const M0_REQ_CMD: u8 = 0x70;
const M0_RESP_CMD: u8 = 0x71;
const M0_FLASH_CHUNK_LEN: usize = 256;

pub mod device_inquiry;

pub struct Handler;

impl SysExHandler for Handler {
    fn execute(app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if device_inquiry::Handler::execute(app, port, data) {
            return true;
        }

        if handle_m0(port, data) {
            return true;
        }

        if led_control::handle_modern(data, LED_SYSEX_DEVICE_ID, &mut LedTarget) {
            return true;
        }

        if app == AppId::Performance && handle_fastled(data) {
            return true;
        }

        DefaultSysExHandler::execute(app, port, data)
    }
}

struct LedTarget;

impl led_control::LedTarget for LedTarget {
    fn set_palette(&mut self, index: u8, velocity: u8) {
        led::set_palette(index, velocity);
    }

    fn set_rgb(&mut self, index: u8, r: u8, g: u8, b: u8) {
        led::set_rgb(index, r, g, b);
    }
}

fn handle_fastled(data: &[u8]) -> bool {
    fastled::handle_targets(data, |target, r, g, b| {
        set_fastled_target(target, r, g, b);
    })
}

fn set_fastled_target(target: u8, r: u8, g: u8, b: u8) {
    match target {
        0 => {
            for index in 0..99 {
                led::set_rgb(index, r, g, b);
            }
        }
        1..=8 => {
            driver::set_rgb_led(100 + target, r, g, b);
            led::set_rgb(target, r, g, b);
        }
        9..=99 => led::set_rgb(target, r, g, b),
        100..=109 => {
            let start = (target - 100) * 10 + 1;
            for index in start..start + 8 {
                led::set_rgb(index, r, g, b);
            }
        }
        110..=119 => {
            let start = target - 100;
            for index in (start..90).step_by(10) {
                led::set_rgb(index, r, g, b);
            }
        }
        _ => {}
    }
}

fn handle_m0(port: MidiPort, data: &[u8]) -> bool {
    if data.len() < 8 || data.last() != Some(&0xf7) {
        return false;
    }
    if !data.starts_with(&NOVATION_HEADER) || data[6] != M0_REQ_CMD {
        return false;
    }

    match data[7] {
        b'S' => handle_status(port),
        b'C' => handle_cached_status(port),
        b'F' => handle_flash_info(port),
        b'B' => handle_flash_begin(port, data),
        b'D' => handle_flash_data(port, data),
        b'V' => handle_flash_verify(port, data),
        b'O' => handle_boot(port),
        b'T' => handle_roadrunner_stats(port),
        _ => send_simple_response(port, data[7], M0_ROM_STATUS_ARG),
    }

    true
}

fn handle_cached_status(port: MidiPort) {
    match runtime::with_runtime(|driver| driver.cached_m0_firmware_status()) {
        Some(status) => send_status_response(port, b'C', &status),
        None => send_simple_response(port, b'C', M0_ROM_STATUS_ARG),
    }
}

fn handle_flash_info(port: MidiPort) {
    match runtime::with_runtime(|driver| driver.flash_info()) {
        Some(info) => send_flash_info_response(port, info.present, &info.jedec_id, info.status1),
        None => send_simple_response(port, b'F', M0_ROM_STATUS_ARG),
    }
}

fn handle_status(port: MidiPort) {
    match runtime::with_runtime(|driver| driver.refresh_m0_firmware_status()) {
        Some(status) => send_status_response(port, b'S', &status),
        None => send_simple_response(port, b'S', M0_ROM_STATUS_ARG),
    }
}

fn handle_roadrunner_stats(port: MidiPort) {
    match runtime::with_runtime(|driver| driver.roadrunner_stats()) {
        Some(Some(stats)) => send_roadrunner_stats_response(port, stats),
        _ => send_simple_response(port, b'T', M0_ROM_STATUS_RX),
    }
}

fn handle_flash_begin(port: MidiPort, data: &[u8]) {
    let Some(_base_addr) = parse_hex(data, 8, 8) else {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    };
    let Some(total_len) = parse_hex(data, 16, 8) else {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    };
    if total_len == 0 {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    }

    let status = runtime::with_m0(|link| {
        let probe = link.force_rom_probe();
        if probe.status != M0_ROM_STATUS_OK {
            return probe.status;
        }
        link.rom_mass_erase()
    })
    .unwrap_or(M0_ROM_STATUS_ARG);
    send_simple_response(port, b'B', status);
}

fn handle_flash_data(port: MidiPort, data: &[u8]) {
    let Some(chunk) = parse_chunk(data) else {
        send_chunk_response(port, b'D', M0_ROM_STATUS_ARG, 0, 0);
        return;
    };

    let status = runtime::with_m0(|link| {
        let probe = link.rom_probe();
        if probe.status != M0_ROM_STATUS_OK {
            return probe.status;
        }
        link.rom_write(chunk.addr, &chunk.data[..chunk.len])
    })
    .unwrap_or(M0_ROM_STATUS_ARG);
    let len = if status == M0_ROM_STATUS_OK {
        chunk.len
    } else {
        0
    };
    send_chunk_response(port, b'D', status, chunk.addr, len as u16);
}

fn handle_flash_verify(port: MidiPort, data: &[u8]) {
    let Some(chunk) = parse_chunk(data) else {
        send_chunk_response(port, b'V', M0_ROM_STATUS_ARG, 0, 0);
        return;
    };

    let status = runtime::with_m0(|link| {
        let probe = link.rom_probe();
        if probe.status != M0_ROM_STATUS_OK {
            return probe.status;
        }

        let mut readback = [0u8; M0_FLASH_CHUNK_LEN];
        let status = link.rom_read(chunk.addr, &mut readback[..chunk.len]);
        if status != M0_ROM_STATUS_OK {
            return status;
        }
        if readback[..chunk.len] == chunk.data[..chunk.len] {
            M0_ROM_STATUS_OK
        } else {
            M0_ROM_STATUS_ARG
        }
    })
    .unwrap_or(M0_ROM_STATUS_ARG);
    let len = if status == M0_ROM_STATUS_OK {
        chunk.len
    } else {
        0
    };
    send_chunk_response(port, b'V', status, chunk.addr, len as u16);
}

fn handle_boot(port: MidiPort) {
    let _ = runtime::with_m0(|link| link.set_mode(2));
    match runtime::with_runtime(|driver| driver.refresh_m0_firmware_status()) {
        Some(status) => send_status_response(port, b'O', &status),
        None => send_simple_response(port, b'O', M0_ROM_STATUS_ARG),
    }
}

struct FlashChunk {
    addr: u32,
    len: usize,
    data: [u8; M0_FLASH_CHUNK_LEN],
}

fn parse_chunk(data: &[u8]) -> Option<FlashChunk> {
    let addr = parse_hex(data, 8, 8)?;
    let len = parse_hex(data, 16, 4)? as usize;
    if len == 0 || len > M0_FLASH_CHUNK_LEN {
        return None;
    }
    if 20 + (len * 2) > data.len().saturating_sub(1) {
        return None;
    }

    let mut chunk = [0u8; M0_FLASH_CHUNK_LEN];
    for (i, byte) in chunk[..len].iter_mut().enumerate() {
        *byte = parse_hex(data, 20 + (i * 2), 2)? as u8;
    }
    Some(FlashChunk {
        addr,
        len,
        data: chunk,
    })
}

fn send_status_response(port: MidiPort, cmd: u8, status: &M0FirmwareStatus) {
    let mut resp = [0u8; 128];
    let mut idx = response_prefix(&mut resp, cmd);
    append_hex8(&mut resp, &mut idx, status.status);
    append_hex8(&mut resp, &mut idx, status.kind);
    append_hex8(&mut resp, &mut idx, status.version_major);
    append_hex8(&mut resp, &mut idx, status.version_minor);
    append_hex8(&mut resp, &mut idx, status.version_patch);
    append_probe(&mut resp, &mut idx, &status.probe);
    send_response(port, &mut resp, idx);
}

fn send_flash_info_response(port: MidiPort, present: bool, jedec_id: &[u8; 3], status1: u8) {
    let mut resp = [0u8; 32];
    let mut idx = response_prefix(&mut resp, b'F');
    append_hex8(&mut resp, &mut idx, M0_ROM_STATUS_OK);
    append_hex8(&mut resp, &mut idx, if present { 1 } else { 0 });
    for byte in jedec_id {
        append_hex8(&mut resp, &mut idx, *byte);
    }
    append_hex8(&mut resp, &mut idx, status1);
    send_response(port, &mut resp, idx);
}

fn send_roadrunner_stats_response(port: MidiPort, stats: runtime::RoadrunnerStats) {
    let mut resp = [0u8; 32];
    let mut idx = response_prefix(&mut resp, b'T');
    append_hex8(&mut resp, &mut idx, M0_ROM_STATUS_OK);
    append_hex32(&mut resp, &mut idx, stats.fast_frames);
    append_hex32(&mut resp, &mut idx, stats.commits);
    append_hex32(&mut resp, &mut idx, stats.rx_overruns);
    send_response(port, &mut resp, idx);
}

fn append_probe(resp: &mut [u8], idx: &mut usize, probe: &M0ProbeResult) {
    append_hex8(resp, idx, probe.status);
    append_hex8(resp, idx, probe.read_status);
    append_hex8(resp, idx, probe.ack);
    append_hex32(resp, idx, probe.baud);
    append_hex16(resp, idx, probe.pid);
    append_hex8(resp, idx, probe.blid);
    append_hex8(resp, idx, probe.vector_len);
    for &byte in probe.vector[..probe.vector_len as usize].iter() {
        append_hex8(resp, idx, byte);
    }
}

fn send_simple_response(port: MidiPort, cmd: u8, status: u8) {
    let mut resp = [0u8; 16];
    let mut idx = response_prefix(&mut resp, cmd);
    append_hex8(&mut resp, &mut idx, status);
    send_response(port, &mut resp, idx);
}

fn send_chunk_response(port: MidiPort, cmd: u8, status: u8, addr: u32, len: u16) {
    let mut resp = [0u8; 32];
    let mut idx = response_prefix(&mut resp, cmd);
    append_hex8(&mut resp, &mut idx, status);
    append_hex32(&mut resp, &mut idx, addr);
    append_hex16(&mut resp, &mut idx, len);
    send_response(port, &mut resp, idx);
}

fn response_prefix(resp: &mut [u8], cmd: u8) -> usize {
    resp[..NOVATION_HEADER.len()].copy_from_slice(&NOVATION_HEADER);
    resp[6] = M0_RESP_CMD;
    resp[7] = cmd;
    8
}

fn send_response(port: MidiPort, resp: &mut [u8], idx: usize) {
    resp[idx] = 0xf7;
    firmware_core::driver::send_midi(port, &resp[..idx + 1]);
}

fn parse_hex(data: &[u8], off: usize, digits: usize) -> Option<u32> {
    if off + digits > data.len().saturating_sub(1) {
        return None;
    }

    let mut value = 0u32;
    for &byte in &data[off..off + digits] {
        value = (value << 4) | hex_value(byte)? as u32;
    }
    Some(value)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn append_hex8(resp: &mut [u8], idx: &mut usize, value: u8) {
    append_nibble(resp, idx, value >> 4);
    append_nibble(resp, idx, value);
}

fn append_hex16(resp: &mut [u8], idx: &mut usize, value: u16) {
    append_hex8(resp, idx, (value >> 8) as u8);
    append_hex8(resp, idx, value as u8);
}

fn append_hex32(resp: &mut [u8], idx: &mut usize, value: u32) {
    append_hex16(resp, idx, (value >> 16) as u16);
    append_hex16(resp, idx, value as u16);
}

fn append_nibble(resp: &mut [u8], idx: &mut usize, value: u8) {
    let value = value & 0x0f;
    resp[*idx] = if value < 10 {
        b'0' + value
    } else {
        b'A' + value - 10
    };
    *idx += 1;
}
