// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::app::AppId;
use crate::driver::{self, M0FirmwareStatus, M0ProbeResult, RoadrunnerStats};
use crate::sys::midi::MidiPort;

const NOVATION_HEADER: [u8; 6] = [0xf0, 0x00, 0x20, 0x29, 0x02, 0x0e];
const M0_REQ_CMD: u8 = 0x70;
const M0_RESP_CMD: u8 = 0x71;
const M0_FLASH_CHUNK_LEN: usize = 256;
const M0_FLASH_BASE: u32 = 0x0800_0000;
const M0_FLASH_MAX_LEN: u32 = 32 * 1024;
const M0_FIRMWARE_KIND_LEGACY: u8 = 0;
const M0_FIRMWARE_KIND_ROADRUNNER: u8 = 1;
const M0_STATUS_RESPONSE_MAX_LEN: usize = 73;
const _: () = assert!(
    crate::sys::driver::common::usb::midi::MIDI_TX_MAX_PACKET_COUNT
        >= M0_STATUS_RESPONSE_MAX_LEN.div_ceil(3)
);

pub const M0_ROM_STATUS_OK: u8 = 0;
pub const M0_ROM_STATUS_RX: u8 = 5;
pub const M0_ROM_STATUS_ARG: u8 = 6;

static M0_FLASH_ACTIVE: AtomicBool = AtomicBool::new(false);
static M0_FLASH_BASE_ADDR: AtomicU32 = AtomicU32::new(M0_FLASH_BASE);
static M0_FLASH_TOTAL_LEN: AtomicU32 = AtomicU32::new(0);
static M0_FLASH_NEXT_WRITE: AtomicU32 = AtomicU32::new(M0_FLASH_BASE);
static M0_FLASH_NEXT_VERIFY: AtomicU32 = AtomicU32::new(M0_FLASH_BASE);
static M0_FLASH_LAST_WRITE_ADDR: AtomicU32 = AtomicU32::new(0);
static M0_FLASH_LAST_WRITE_LEN: AtomicU32 = AtomicU32::new(0);
static M0_FLASH_LAST_VERIFY_ADDR: AtomicU32 = AtomicU32::new(0);
static M0_FLASH_LAST_VERIFY_LEN: AtomicU32 = AtomicU32::new(0);
static M0_BOOT_RETRYABLE: AtomicBool = AtomicBool::new(false);
static BOOT_CYCLE_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn execute(_app: AppId, port: MidiPort, data: &[u8]) -> bool {
    handle_m0(port, data)
}

pub fn take_requested_app_switch() -> Option<AppId> {
    BOOT_CYCLE_REQUESTED
        .swap(false, Ordering::AcqRel)
        .then_some(AppId::Boot)
}

fn handle_m0(port: MidiPort, data: &[u8]) -> bool {
    if data.len() < 8 || data.last() != Some(&0xf7) {
        return false;
    }
    if !data.starts_with(&NOVATION_HEADER) || data[6] != M0_REQ_CMD {
        return false;
    }

    match data[7] {
        b'S' if data.len() == 9 => handle_status(port),
        b'C' if data.len() == 9 => handle_cached_status(port),
        b'F' if data.len() == 9 => handle_flash_info(port),
        b'B' => handle_flash_begin(port, data),
        b'D' => handle_flash_data(port, data),
        b'V' => handle_flash_verify(port, data),
        b'O' if data.len() == 9 => handle_boot(port),
        b'T' if data.len() == 9 => handle_roadrunner_stats(port),
        b'S' | b'C' | b'F' | b'O' | b'T' => {
            send_simple_response(port, data[7], M0_ROM_STATUS_ARG)
        }
        _ => send_simple_response(port, data[7], M0_ROM_STATUS_ARG),
    }

    true
}

fn handle_cached_status(port: MidiPort) {
    match driver::cached_m0_firmware_status() {
        Some(status) => send_status_response(port, b'C', &status),
        None => send_simple_response(port, b'C', M0_ROM_STATUS_ARG),
    }
}

fn handle_flash_info(port: MidiPort) {
    match driver::flash_info() {
        Some(info) => send_flash_info_response(port, info.present, &info.jedec_id, info.status1),
        None => send_simple_response(port, b'F', M0_ROM_STATUS_ARG),
    }
}

fn handle_status(port: MidiPort) {
    match driver::refresh_m0_firmware_status() {
        Some(status) => send_status_response(port, b'S', &status),
        None => send_simple_response(port, b'S', M0_ROM_STATUS_ARG),
    }
}

fn handle_roadrunner_stats(port: MidiPort) {
    match driver::roadrunner_stats() {
        Some(Some(stats)) => send_roadrunner_stats_response(port, stats),
        _ => send_simple_response(port, b'T', M0_ROM_STATUS_RX),
    }
}

fn handle_flash_begin(port: MidiPort, data: &[u8]) {
    if data.len() != 25 {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    }
    let Some(base_addr) = parse_hex(data, 8, 8) else {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    };
    let Some(total_len) = parse_hex(data, 16, 8) else {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    };
    if base_addr != M0_FLASH_BASE || !(1..=M0_FLASH_MAX_LEN).contains(&total_len) {
        send_simple_response(port, b'B', M0_ROM_STATUS_ARG);
        return;
    }

    M0_FLASH_ACTIVE.store(false, Ordering::Release);
    M0_BOOT_RETRYABLE.store(false, Ordering::Release);
    BOOT_CYCLE_REQUESTED.store(false, Ordering::Release);
    let status = driver::m0_force_rom_probe();
    if status == M0_ROM_STATUS_OK {
        M0_FLASH_BASE_ADDR.store(base_addr, Ordering::Relaxed);
        M0_FLASH_TOTAL_LEN.store(total_len, Ordering::Relaxed);
        M0_FLASH_NEXT_WRITE.store(base_addr, Ordering::Relaxed);
        M0_FLASH_NEXT_VERIFY.store(base_addr, Ordering::Relaxed);
        M0_FLASH_LAST_WRITE_ADDR.store(0, Ordering::Relaxed);
        M0_FLASH_LAST_WRITE_LEN.store(0, Ordering::Relaxed);
        M0_FLASH_LAST_VERIFY_ADDR.store(0, Ordering::Relaxed);
        M0_FLASH_LAST_VERIFY_LEN.store(0, Ordering::Relaxed);
        M0_FLASH_ACTIVE.store(true, Ordering::Release);
    }
    send_simple_response(port, b'B', status);
}

fn handle_flash_data(port: MidiPort, data: &[u8]) {
    let Some(chunk) = parse_chunk(data) else {
        send_chunk_response(port, b'D', M0_ROM_STATUS_ARG, 0, 0);
        return;
    };

    let status = if !flash_chunk_in_range(chunk.addr, chunk.len) {
        M0_ROM_STATUS_ARG
    } else if !M0_FLASH_ACTIVE.load(Ordering::Acquire) {
        M0_ROM_STATUS_RX
    } else {
        let next = M0_FLASH_NEXT_WRITE.load(Ordering::Acquire);
        let last_addr = M0_FLASH_LAST_WRITE_ADDR.load(Ordering::Acquire);
        let last_len = M0_FLASH_LAST_WRITE_LEN.load(Ordering::Acquire);
        if chunk.addr == next {
            let status = driver::m0_rom_write(chunk.addr, &chunk.data[..chunk.len]);
            if status == M0_ROM_STATUS_OK {
                M0_FLASH_NEXT_WRITE.store(
                    chunk.addr + chunk.len as u32,
                    Ordering::Release,
                );
                M0_FLASH_LAST_WRITE_ADDR.store(chunk.addr, Ordering::Relaxed);
                M0_FLASH_LAST_WRITE_LEN.store(chunk.len as u32, Ordering::Relaxed);
            }
            status
        } else if chunk.addr == last_addr && chunk.len as u32 == last_len {
            // If the response to a successful write was lost, replaying the
            // same chunk is safe only after confirming flash contains it.
            verify_chunk_bytes(&chunk)
        } else {
            M0_ROM_STATUS_ARG
        }
    };
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

    let status = if !flash_chunk_in_range(chunk.addr, chunk.len) {
        M0_ROM_STATUS_ARG
    } else if !M0_FLASH_ACTIVE.load(Ordering::Acquire) {
        M0_ROM_STATUS_RX
    } else {
        let next = M0_FLASH_NEXT_VERIFY.load(Ordering::Acquire);
        let last_addr = M0_FLASH_LAST_VERIFY_ADDR.load(Ordering::Acquire);
        let last_len = M0_FLASH_LAST_VERIFY_LEN.load(Ordering::Acquire);
        let next_write = M0_FLASH_NEXT_WRITE.load(Ordering::Acquire);
        let chunk_end = chunk.addr + chunk.len as u32;
        if chunk_end > next_write {
            M0_ROM_STATUS_RX
        } else if chunk.addr == next {
            let status = verify_chunk_bytes(&chunk);
            if status == M0_ROM_STATUS_OK {
                M0_FLASH_NEXT_VERIFY.store(chunk_end, Ordering::Release);
                M0_FLASH_LAST_VERIFY_ADDR.store(chunk.addr, Ordering::Relaxed);
                M0_FLASH_LAST_VERIFY_LEN.store(chunk.len as u32, Ordering::Relaxed);
            }
            status
        } else if chunk.addr == last_addr && chunk.len as u32 == last_len {
            // Verification requests are also idempotent for a retried MIDI
            // transaction after the original response was lost.
            verify_chunk_bytes(&chunk)
        } else {
            M0_ROM_STATUS_ARG
        }
    };
    let len = if status == M0_ROM_STATUS_OK {
        chunk.len
    } else {
        0
    };
    send_chunk_response(port, b'V', status, chunk.addr, len as u16);
}

fn handle_boot(port: MidiPort) {
    if !flash_session_complete() {
        if M0_BOOT_RETRYABLE.load(Ordering::Acquire) {
            if let Some(status) = driver::cached_m0_firmware_status() {
                send_status_response(port, b'O', &status);
                return;
            }
        }
        let mut status = driver::cached_m0_firmware_status()
            .unwrap_or_else(M0FirmwareStatus::unknown);
        status.status = M0_ROM_STATUS_ARG;
        send_status_response(port, b'O', &status);
        return;
    }

    M0_BOOT_RETRYABLE.store(false, Ordering::Release);
    M0_FLASH_ACTIVE.store(false, Ordering::Release);
    driver::m0_set_mode(2);
    match driver::refresh_m0_firmware_status() {
        Some(status) => {
            let known_kind = matches!(
                status.kind,
                M0_FIRMWARE_KIND_LEGACY | M0_FIRMWARE_KIND_ROADRUNNER
            );
            let firmware_started = status.status == M0_ROM_STATUS_OK && known_kind;
            let mut response = status;
            if response.status == M0_ROM_STATUS_OK && !known_kind {
                response.status = M0_ROM_STATUS_ARG;
            }
            send_status_response(port, b'O', &response);
            if firmware_started {
                M0_BOOT_RETRYABLE.store(true, Ordering::Release);
                BOOT_CYCLE_REQUESTED.store(true, Ordering::Release);
            }
        }
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
    if data.len() != 21 + (len * 2) {
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

fn flash_chunk_in_range(addr: u32, len: usize) -> bool {
    if len == 0 || len > M0_FLASH_CHUNK_LEN {
        return false;
    }
    let base = M0_FLASH_BASE_ADDR.load(Ordering::Acquire);
    let total = M0_FLASH_TOTAL_LEN.load(Ordering::Acquire);
    let Some(end) = base.checked_add(total) else {
        return false;
    };
    addr >= base && (len as u32) <= end.saturating_sub(addr)
}

fn flash_session_complete() -> bool {
    if !M0_FLASH_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let base = M0_FLASH_BASE_ADDR.load(Ordering::Acquire);
    let total = M0_FLASH_TOTAL_LEN.load(Ordering::Acquire);
    let Some(end) = base.checked_add(total) else {
        return false;
    };
    M0_FLASH_NEXT_WRITE.load(Ordering::Acquire) == end
        && M0_FLASH_NEXT_VERIFY.load(Ordering::Acquire) == end
}

fn verify_chunk_bytes(chunk: &FlashChunk) -> u8 {
    let mut readback = [0u8; M0_FLASH_CHUNK_LEN];
    let status = driver::m0_rom_read(chunk.addr, &mut readback[..chunk.len]);
    if status == M0_ROM_STATUS_OK && readback[..chunk.len] == chunk.data[..chunk.len] {
        M0_ROM_STATUS_OK
    } else if status == M0_ROM_STATUS_OK {
        M0_ROM_STATUS_ARG
    } else {
        status
    }
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

fn send_roadrunner_stats_response(port: MidiPort, stats: RoadrunnerStats) {
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
    driver::send_midi(port, &resp[..idx + 1]);
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
