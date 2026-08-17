// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cell::UnsafeCell;
use core::ptr;

use super::hw;
use crate::app::{MidiEvent, MidiPort};

const USB_BASE: usize = 0x4000_5c00;
const USB_PMA: usize = 0x4000_6000;

const USB_CNTR: *mut u32 = (USB_BASE + 0x40) as *mut u32;
const USB_ISTR: *mut u32 = (USB_BASE + 0x44) as *mut u32;
const USB_DADDR: *mut u32 = (USB_BASE + 0x4c) as *mut u32;
const USB_BTABLE: *mut u32 = (USB_BASE + 0x50) as *mut u32;

const ISTR_CTR: u16 = 1 << 15;
#[allow(dead_code)]
const ISTR_ERR: u16 = 1 << 13;
#[allow(dead_code)]
const ISTR_WKUP: u16 = 1 << 12;
#[allow(dead_code)]
const ISTR_SUSP: u16 = 1 << 11;
const ISTR_RESET: u16 = 1 << 10;
const ISTR_EP_ID: u16 = 0x000f;

const CNTR_FRES: u16 = 1 << 0;
const CNTR_CTRM: u16 = 1 << 15;
const CNTR_ERRM: u16 = 1 << 13;
const CNTR_WKUPM: u16 = 1 << 12;
const CNTR_SUSPM: u16 = 1 << 11;
const CNTR_RESETM: u16 = 1 << 10;
const DADDR_EF: u16 = 1 << 7;

const EP_CTR_RX: u16 = 1 << 15;
const EP_DTOG_RX: u16 = 1 << 14;
const EP_STAT_RX: u16 = 0b11 << 12;
const EP_SETUP: u16 = 1 << 11;
const EP_TYPE: u16 = 0b11 << 9;
const EP_KIND: u16 = 1 << 8;
const EP_TYPE_CONTROL: u16 = 0b01 << 9;
const EP_TYPE_INTERRUPT: u16 = 0b11 << 9;
const EP_CTR_TX: u16 = 1 << 7;
const EP_DTOG_TX: u16 = 1 << 6;
const EP_STAT_TX: u16 = 0b11 << 4;
const EP_ADDR: u16 = 0x0f;

const EP_STAT_DISABLED: u16 = 0b00;
const EP_STAT_STALL: u16 = 0b01;
const EP_STAT_NAK: u16 = 0b10;
const EP_STAT_VALID: u16 = 0b11;

const EP0: usize = 0;
const EP1_IN: usize = 1;
const EP2_OUT: usize = 2;

const EP0_TX_ADDR: u16 = 0x40;
const EP0_RX_ADDR: u16 = 0x80;
const EP1_TX_ADDR: u16 = 0xc0;
const EP2_RX_ADDR: u16 = 0x100;
const EP_MAX_PACKET: usize = 64;
const MIDI_RX_QUEUE_SIZE: usize = 64;
const MIDI_TX_QUEUE_SIZE: usize = 64;
const MIDI_TX_MAX_PACKET_COUNT: usize = EP_MAX_PACKET / 4;

const REQ_GET_STATUS: u8 = 0x00;
const REQ_CLEAR_FEATURE: u8 = 0x01;
const REQ_SET_ADDRESS: u8 = 0x05;
const REQ_GET_DESCRIPTOR: u8 = 0x06;
const REQ_GET_CONFIGURATION: u8 = 0x08;
const REQ_SET_CONFIGURATION: u8 = 0x09;
const REQ_GET_INTERFACE: u8 = 0x0a;
const REQ_SET_INTERFACE: u8 = 0x0b;

const DESC_DEVICE: u8 = 0x01;
const DESC_CONFIGURATION: u8 = 0x02;
const DESC_STRING: u8 = 0x03;

#[cfg(feature = "launchpad-mini-mk1")]
const USB_PRODUCT_ID: u8 = 0x36;
#[cfg(not(feature = "launchpad-mini-mk1"))]
const USB_PRODUCT_ID: u8 = 0x20;

const DEVICE_DESCRIPTOR: [u8; 18] = [
    18,
    DESC_DEVICE,
    0x10,
    0x01,
    0x00,
    0x00,
    0x00,
    64,
    0x35,
    0x12,
    USB_PRODUCT_ID,
    0x00,
    0x00,
    0x00,
    1,
    2,
    4, // serial number index is 4
    1,
];

const CONFIG_TOTAL_LEN: u16 = 129;
const MIDI_STREAMING_TOTAL_LEN: u16 = 93;

const CONFIG_DESCRIPTOR: [u8; CONFIG_TOTAL_LEN as usize] = [
    // Configuration descriptor (9 bytes)
    9,
    DESC_CONFIGURATION,
    CONFIG_TOTAL_LEN as u8,
    0,
    2, // 2 interfaces
    1,
    0,
    0x80,
    30,
    // Audio Control Interface Descriptor (9 bytes)
    9,
    4,
    0,
    0,
    0,
    1,
    1,
    0,
    3, // interface string index 3 (STRING_INTERFACE)
    // Class-specific AC Interface Descriptor (9 bytes)
    9,
    0x24,
    1,
    0x00,
    0x01,
    9,
    0,
    1,
    1,
    // MIDI Streaming Interface Descriptor (9 bytes)
    9,
    4,
    1,
    0,
    2, // 2 endpoints
    1,
    3,
    0,
    3, // interface string index 3 (STRING_INTERFACE)
    // Class-specific MS Interface Header (7 bytes)
    7,
    0x24,
    1,
    0x00,
    0x01,
    MIDI_STREAMING_TOTAL_LEN as u8,
    0,
    // --- MIDI IN Jacks ---
    // 1. External IN jack 1 (ID 1)
    6,
    0x24,
    2,
    1, // External
    1, // Jack ID
    0,
    // 2. External IN jack 2 (ID 3)
    6,
    0x24,
    2,
    1, // External
    3, // Jack ID
    0,
    // 3. Embedded IN jack 1 (ID 6)
    6,
    0x24,
    2,
    1, // Embedded
    6, // Jack ID
    5, // String index for DAW
    // 4. Embedded IN jack 2 (ID 8)
    6,
    0x24,
    2,
    1, // Embedded
    8, // Jack ID
    6, // String index for MIDI
    // --- MIDI OUT Jacks ---
    // 1. External OUT jack 1 (ID 5)
    9,
    0x24,
    3,
    1, // External
    5, // Jack ID
    1, // Number of input pins
    6, // Input pin source: Embedded IN jack 1 (ID 6)
    1, // Input pin source outlet
    0,
    // 2. External OUT jack 2 (ID 7)
    9,
    0x24,
    3,
    1, // External
    7, // Jack ID
    1, // Number of input pins
    8, // Input pin source: Embedded IN jack 2 (ID 8)
    1, // Input pin source outlet
    0,
    // 3. Embedded OUT jack 1 (ID 2)
    9,
    0x24,
    3,
    1, // Embedded
    2, // Jack ID
    1, // Number of input pins
    1, // Input pin source: External IN jack 1 (ID 1)
    1, // Input pin source outlet
    5, // String index for DAW
    // 4. Embedded OUT jack 2 (ID 4)
    9,
    0x24,
    3,
    1, // Embedded
    4, // Jack ID
    1, // Number of input pins
    3, // Input pin source: External IN jack 2 (ID 3)
    1, // Input pin source outlet
    6, // String index for MIDI
    // --- Endpoints ---
    // Bulk OUT Endpoint 2 (7 bytes)
    7,
    5,
    0x02, // Endpoint 2 OUT
    3,    // Bulk
    64,   // Max packet size
    0,
    1,
    // Class-specific MS Bulk OUT Endpoint Descriptor (6 bytes)
    6,
    0x25,
    1, // MS_GENERAL
    2, // 2 jacks
    6, // Embedded IN jack 1 (ID 6)
    8, // Embedded IN jack 2 (ID 8)
    // Bulk IN Endpoint 1 (7 bytes)
    7,
    5,
    0x81, // Endpoint 1 IN
    3,    // Bulk
    64,   // Max packet size
    0,
    1,
    // Class-specific MS Bulk IN Endpoint Descriptor (6 bytes)
    6,
    0x25,
    1, // MS_GENERAL
    2, // 2 jacks
    2, // Embedded OUT jack 1 (ID 2)
    4, // Embedded OUT jack 2 (ID 4)
];

const STRING_ZERO: [u8; 4] = [4, DESC_STRING, 0x09, 0x04];
const STRING_MANUFACTURER: [u8; 38] = utf16_string_18(*b"Focusrite A.E. Ltd");

#[cfg(feature = "launchpad-mini-mk1")]
const STRING_PRODUCT: [u8; 30] = utf16_string_14(*b"Launchpad Mini");
#[cfg(not(feature = "launchpad-mini-mk1"))]
const STRING_PRODUCT: [u8; 24] = utf16_string_11(*b"Launchpad S");

#[cfg(feature = "launchpad-mini-mk1")]
const STRING_INTERFACE: [u8; 30] = utf16_string_14(*b"Launchpad Mini");
#[cfg(not(feature = "launchpad-mini-mk1"))]
const STRING_INTERFACE: [u8; 24] = utf16_string_11(*b"Launchpad S");

#[cfg(feature = "launchpad-mini-mk1")]
const STRING_SERIAL: [u8; 24] = utf16_string_11(*b"COREFW-MINI");
#[cfg(not(feature = "launchpad-mini-mk1"))]
const STRING_SERIAL: [u8; 22] = utf16_string_10(*b"COREFW-LPS");

#[cfg(feature = "launchpad-mini-mk1")]
const STRING_JACK_DAW: [u8; 22] = utf16_string_10(*b"MINI (DAW)");
#[cfg(not(feature = "launchpad-mini-mk1"))]
const STRING_JACK_DAW: [u8; 20] = utf16_string_9(*b"LPS (DAW)");

#[cfg(feature = "launchpad-mini-mk1")]
const STRING_JACK_MIDI: [u8; 24] = utf16_string_11(*b"MINI (MIDI)");
#[cfg(not(feature = "launchpad-mini-mk1"))]
const STRING_JACK_MIDI: [u8; 22] = utf16_string_10(*b"LPS (MIDI)");

#[allow(dead_code)]
const fn utf16_string_9(value: [u8; 9]) -> [u8; 20] {
    let mut out = [0u8; 20];
    out[0] = 20;
    out[1] = DESC_STRING;
    let mut i = 0;
    while i < 9 {
        out[2 + i * 2] = value[i];
        i += 1;
    }
    out
}

#[allow(dead_code)]
const fn utf16_string_10(value: [u8; 10]) -> [u8; 22] {
    let mut out = [0u8; 22];
    out[0] = 22;
    out[1] = DESC_STRING;
    let mut i = 0;
    while i < 10 {
        out[2 + i * 2] = value[i];
        i += 1;
    }
    out
}

#[allow(dead_code)]
const fn utf16_string_11(value: [u8; 11]) -> [u8; 24] {
    let mut out = [0u8; 24];
    out[0] = 24;
    out[1] = DESC_STRING;
    let mut i = 0;
    while i < 11 {
        out[2 + i * 2] = value[i];
        i += 1;
    }
    out
}

#[allow(dead_code)]
const fn utf16_string_14(value: [u8; 14]) -> [u8; 30] {
    let mut out = [0u8; 30];
    out[0] = 30;
    out[1] = DESC_STRING;
    let mut i = 0;
    while i < 14 {
        out[2 + i * 2] = value[i];
        i += 1;
    }
    out
}

#[allow(dead_code)]
const fn utf16_string_18(value: [u8; 18]) -> [u8; 38] {
    let mut out = [0u8; 38];
    out[0] = 38;
    out[1] = DESC_STRING;
    let mut i = 0;
    while i < 18 {
        out[2 + i * 2] = value[i];
        i += 1;
    }
    out
}

#[derive(Copy, Clone)]
#[repr(C)]
struct UsbMidiPacket {
    data: [u8; 4],
}

const EMPTY_MIDI_EVENT: MidiEvent = MidiEvent {
    port: MidiPort::Daw,
    status: 0,
    data1: 0,
    data2: 0,
};
const EMPTY_USB_MIDI_PACKET: UsbMidiPacket = UsbMidiPacket { data: [0; 4] };

struct Ring<T, const N: usize> {
    buffer: [T; N],
    head: usize,
    tail: usize,
}

impl<T: Copy, const N: usize> Ring<T, N> {
    const fn new(init: T) -> Self {
        Self {
            buffer: [init; N],
            head: 0,
            tail: 0,
        }
    }

    fn push(&mut self, item: T) -> bool {
        let next = (self.head + 1) % N;
        if next == self.tail {
            return false;
        }

        self.buffer[self.head] = item;
        self.head = next;
        true
    }

    fn pop(&mut self) -> Option<T> {
        if self.head == self.tail {
            return None;
        }

        let item = self.buffer[self.tail];
        self.tail = (self.tail + 1) % N;
        Some(item)
    }
}



struct UsbState {
    tx_queue: Ring<UsbMidiPacket, MIDI_TX_QUEUE_SIZE>,
    rx_queue: Ring<MidiEvent, MIDI_RX_QUEUE_SIZE>,
    pending_address: u8,
    configuration: u8,
    tx_data: *const u8,
    tx_len: usize,
    tx_pos: usize,
    tx_zlp: bool,
    control_buf: [u8; 64],
}

impl UsbState {
    const fn new() -> Self {
        Self {
            tx_queue: Ring::new(EMPTY_USB_MIDI_PACKET),
            rx_queue: Ring::new(EMPTY_MIDI_EVENT),
            pending_address: 0,
            configuration: 0,
            tx_data: core::ptr::null(),
            tx_len: 0,
            tx_pos: 0,
            tx_zlp: false,
            control_buf: [0; 64],
        }
    }
}

struct UsbStateCell(UnsafeCell<UsbState>);
unsafe impl Sync for UsbStateCell {}

static STATE: UsbStateCell = UsbStateCell(UnsafeCell::new(UsbState::new()));

pub fn init() {
    hw::pac::RCC.apb1enr().modify(|w| w.set_usben(true));
    hw::pac::RCC.apb1rstr().modify(|w| w.set_usbrst(true));
    for _ in 0..16 {
        cortex_m::asm::nop();
    }
    hw::pac::RCC.apb1rstr().modify(|w| w.set_usbrst(false));
    unsafe {
        write16(USB_CNTR, CNTR_FRES);
        for _ in 0..128 {
            cortex_m::asm::nop();
        }
        write16(USB_CNTR, 0);
        write16(USB_ISTR, 0);
        write16(USB_BTABLE, 0);
        reset_bus();
        write16(
            USB_CNTR,
            CNTR_CTRM | CNTR_ERRM | CNTR_WKUPM | CNTR_SUSPM | CNTR_RESETM,
        );
        cortex_m::peripheral::NVIC::unmask(hw::Interrupt::USB_LP_CAN1_RX0);
    }
}

pub fn poll() {
    loop {
        let istr = unsafe { read16(USB_ISTR) };
        if istr & ISTR_RESET != 0 {
            unsafe {
                write16(USB_ISTR, !ISTR_RESET);
                reset_bus();
            }
        }
        if istr & ISTR_CTR != 0 {
            let ep = (istr & ISTR_EP_ID) as usize;
            service_ep(ep);
        }
        if istr & (ISTR_RESET | ISTR_CTR) == 0 {
            break;
        }
    }
    pump_tx();
}

pub fn dequeue_midi_event() -> Option<MidiEvent> {
    let state = unsafe { &mut *STATE.0.get() };
    state.rx_queue.pop()
}

pub fn enqueue_tx_message(port: u8, data: &[u8]) -> bool {
    let state = unsafe { &mut *STATE.0.get() };
    let cable = match port {
        0 => 0x00,
        1 => 0x10,
        _ => 0x00,
    };
    let mut temp = [EMPTY_USB_MIDI_PACKET; MIDI_TX_MAX_PACKET_COUNT];
    let count = match convert_midi_stream_to_packets(data, cable, &mut temp) {
        Ok(c) => c,
        Err(_) => return false,
    };
    for packet in temp.iter().take(count) {
        if !state.tx_queue.push(*packet) {
            return false;
        }
    }
    pump_tx();
    true
}

fn pump_tx() {
    let state = unsafe { &mut *STATE.0.get() };
    if ep_reg(EP1_IN) & EP_STAT_TX == EP_STAT_VALID {
        return;
    }
    let mut payload = [0u8; EP_MAX_PACKET];
    let mut count = 0;
    while count + 4 <= EP_MAX_PACKET {
        let Some(packet) = state.tx_queue.pop() else {
            break;
        };
        payload[count..count + 4].copy_from_slice(&packet.data);
        count += 4;
    }
    if count > 0 {
        write_ep_tx(EP1_IN, EP1_TX_ADDR, &payload[..count]);
        unsafe {
            set_tx_stat(EP1_IN, EP_STAT_VALID);
        }
    }
}

fn reset_bus() {
    unsafe {
        write16(USB_DADDR, DADDR_EF);
        set_btable(EP0, EP0_TX_ADDR, 0, EP0_RX_ADDR, rx_count(EP_MAX_PACKET));
        set_btable(EP1_IN, EP1_TX_ADDR, 0, 0, 0);
        set_btable(EP2_OUT, 0, 0, EP2_RX_ADDR, rx_count(EP_MAX_PACKET));
        set_ep_reg(EP0, EP_TYPE_CONTROL | 0, EP_STAT_NAK, EP_STAT_VALID);
        set_ep_reg(EP1_IN, EP_TYPE_INTERRUPT | 1, EP_STAT_NAK, EP_STAT_DISABLED);
        set_ep_reg(
            EP2_OUT,
            EP_TYPE_INTERRUPT | 2,
            EP_STAT_DISABLED,
            EP_STAT_VALID,
        );
    }
}

fn service_ep(ep: usize) {
    let reg = ep_reg(ep);
    if reg & EP_CTR_RX != 0 {
        if ep == EP0 {
            if reg & EP_SETUP != 0 {
                let mut setup = [0u8; 8];
                pma_read(EP0_RX_ADDR, &mut setup);
                unsafe {
                    clear_ctr(EP0, true, false);
                }
                handle_setup(setup);
            } else {
                unsafe {
                    clear_ctr(EP0, true, false);
                    set_rx_stat(EP0, EP_STAT_VALID);
                }
            }
        } else if ep == EP2_OUT {
            let rx_count = (unsafe { read_pma_u16(0x06) } & 0x03ff) as usize;
            let mut buf = [0u8; EP_MAX_PACKET];
            let len = rx_count.min(EP_MAX_PACKET);
            pma_read(EP2_RX_ADDR, &mut buf[..len]);
            unsafe {
                clear_ctr(EP2_OUT, true, false);
                set_rx_stat(EP2_OUT, EP_STAT_VALID);
            }
            parse_usb_midi_packets(&buf[..len]);
        }
    }

    if reg & EP_CTR_TX != 0 {
        unsafe {
            clear_ctr(ep, false, true);
        }
        if ep == EP0 {
            let state = unsafe { &mut *STATE.0.get() };
            if state.pending_address != 0 {
                unsafe {
                    write16(USB_DADDR, DADDR_EF | state.pending_address as u16);
                }
                state.pending_address = 0;
            } else if state.tx_pos < state.tx_len || state.tx_zlp {
                send_next_ep0_packet();
            } else {
                unsafe {
                    set_rx_stat(EP0, EP_STAT_VALID);
                }
            }
        } else if ep == EP1_IN {
            pump_tx();
        }
    }
}

fn parse_usb_midi_packets(buf: &[u8]) {
    let state = unsafe { &mut *STATE.0.get() };
    let mut offset = 0;
    while offset + 4 <= buf.len() {
        let packet = &buf[offset..offset + 4];
        let cin = packet[0] & 0x0f;
        let cable = packet[0] >> 4;
        let port = match cable {
            0 => MidiPort::Daw,
            1 => MidiPort::Midi,
            _ => MidiPort::Din,
        };
        let len = match cin {
            0x5 | 0xf => 1,
            0x2 | 0x6 | 0xc | 0xd => 2,
            0x3 | 0x4 | 0x7 | 0x8 | 0x9 | 0xa | 0xb | 0xe => 3,
            _ => 0,
        };
        if len > 0 {
            let _ = state.rx_queue.push(MidiEvent {
                port,
                status: packet[1],
                data1: if len > 1 { packet[2] } else { 0 },
                data2: if len > 2 { packet[3] } else { 0 },
            });
        }
        offset += 4;
    }
}

fn convert_midi_stream_to_packets(
    data: &[u8],
    cable: u8,
    out: &mut [UsbMidiPacket; MIDI_TX_MAX_PACKET_COUNT],
) -> Result<usize, ()> {
    if data.is_empty() {
        return Ok(0);
    }

    let mut src = 0;
    let mut dst = 0;

    while src < data.len() {
        if dst >= out.len() {
            return Err(());
        }

        let status = data[src];

        if status == 0xf0 {
            let mut sysex_len = 0;
            while src + sysex_len < data.len() && data[src + sysex_len] != 0xf7 {
                sysex_len += 1;
            }
            if src + sysex_len < data.len() && data[src + sysex_len] == 0xf7 {
                sysex_len += 1;
            }

            let mut sub_src = 0;
            while sub_src < sysex_len {
                if dst >= out.len() {
                    return Err(());
                }

                let rem = sysex_len - sub_src;
                let take = rem.min(3);
                let cin = match take {
                    1 => 0x05,
                    2 => 0x06,
                    3 => {
                        if sub_src + 3 == sysex_len && data[src + sub_src + 2] == 0xf7 {
                            0x07
                        } else {
                            0x04
                        }
                    }
                    _ => unreachable!(),
                };

                out[dst] = UsbMidiPacket {
                    data: [
                        cable | cin,
                        data[src + sub_src],
                        if take > 1 {
                            data[src + sub_src + 1]
                        } else {
                            0
                        },
                        if take > 2 {
                            data[src + sub_src + 2]
                        } else {
                            0
                        },
                    ],
                };

                sub_src += take;
                dst += 1;
            }

            src += sysex_len;
            continue;
        }

        let (cin, len) = match short_message_format(status) {
            Some(x) => x,
            None => {
                src += 1;
                continue;
            }
        };

        if src + len > data.len() {
            break;
        }

        let take = len.min(data.len() - src);

        out[dst] = UsbMidiPacket {
            data: [
                cable | cin,
                data[src],
                if take > 1 { data[src + 1] } else { 0 },
                if take > 2 { data[src + 2] } else { 0 },
            ],
        };
        src += take;
        dst += 1;
    }

    Ok(dst)
}

fn short_message_format(status: u8) -> Option<(u8, usize)> {
    match status {
        0x80..=0x8f => Some((0x8, 3)),
        0x90..=0x9f => Some((0x9, 3)),
        0xa0..=0xaf => Some((0xa, 3)),
        0xb0..=0xbf => Some((0xb, 3)),
        0xc0..=0xcf => Some((0xc, 2)),
        0xd0..=0xdf => Some((0xd, 2)),
        0xe0..=0xef => Some((0xe, 3)),
        0xf1 | 0xf3 => Some((0x2, 2)),
        0xf2 => Some((0x3, 3)),
        0xf6 => Some((0x5, 1)),
        0xf8..=0xff => Some((0xf, 1)),
        _ => None,
    }
}

fn handle_setup(setup: [u8; 8]) {
    let bm_request_type = setup[0];
    let request = setup[1];
    let value = u16::from_le_bytes([setup[2], setup[3]]);
    let index = u16::from_le_bytes([setup[4], setup[5]]);
    let length = u16::from_le_bytes([setup[6], setup[7]]) as usize;

    if bm_request_type & 0x60 != 0 {
        stall_ep0();
        return;
    }

    match (bm_request_type & 0x80 != 0, request) {
        (true, REQ_GET_DESCRIPTOR) => {
            let desc_type = (value >> 8) as u8;
            let desc_index = value as u8;
            if let Some(data) = descriptor(desc_type, desc_index) {
                start_control_read(data, length);
            } else {
                stall_ep0();
            }
        }
        (false, REQ_SET_ADDRESS) => {
            let state = unsafe { &mut *STATE.0.get() };
            state.pending_address = (value & 0x7f) as u8;
            status_in();
        }
        (false, REQ_SET_CONFIGURATION) if value <= 1 => {
            let state = unsafe { &mut *STATE.0.get() };
            state.configuration = value as u8;
            if state.configuration != 0 {
                unsafe {
                    set_tx_stat(EP1_IN, EP_STAT_NAK);
                    set_rx_stat(EP2_OUT, EP_STAT_VALID);
                }
            }
            status_in();
        }
        (true, REQ_GET_CONFIGURATION) => {
            let state = unsafe { &mut *STATE.0.get() };
            state.control_buf[0] = state.configuration;
            start_control_read_ptr(state.control_buf.as_ptr(), 1, length);
        }
        (true, REQ_GET_STATUS) => {
            let state = unsafe { &mut *STATE.0.get() };
            state.control_buf[0] = 0;
            state.control_buf[1] = 0;
            start_control_read_ptr(state.control_buf.as_ptr(), 2, length);
        }
        (false, REQ_CLEAR_FEATURE) => status_in(),
        (true, REQ_GET_INTERFACE) if index <= 1 => {
            let state = unsafe { &mut *STATE.0.get() };
            state.control_buf[0] = 0;
            start_control_read_ptr(state.control_buf.as_ptr(), 1, length);
        }
        (false, REQ_SET_INTERFACE) if index <= 1 && value == 0 => status_in(),
        _ => stall_ep0(),
    }
}

fn descriptor(desc_type: u8, index: u8) -> Option<&'static [u8]> {
    match (desc_type, index) {
        (DESC_DEVICE, 0) => Some(&DEVICE_DESCRIPTOR),
        (DESC_CONFIGURATION, 0) => Some(&CONFIG_DESCRIPTOR),
        (DESC_STRING, 0) => Some(&STRING_ZERO),
        (DESC_STRING, 1) => Some(&STRING_MANUFACTURER),
        (DESC_STRING, 2) => Some(&STRING_PRODUCT),
        (DESC_STRING, 3) => Some(&STRING_INTERFACE),
        (DESC_STRING, 4) => Some(&STRING_SERIAL),
        (DESC_STRING, 5) => Some(&STRING_JACK_DAW),
        (DESC_STRING, 6) => Some(&STRING_JACK_MIDI),
        _ => None,
    }
}

fn start_control_read(data: &'static [u8], requested_len: usize) {
    start_control_read_ptr(data.as_ptr(), data.len(), requested_len);
}

fn start_control_read_ptr(data: *const u8, data_len: usize, requested_len: usize) {
    let state = unsafe { &mut *STATE.0.get() };
    state.tx_data = data;
    state.tx_len = data_len.min(requested_len);
    state.tx_pos = 0;
    state.tx_zlp =
        state.tx_len != 0 && state.tx_len % EP_MAX_PACKET == 0 && requested_len > state.tx_len;
    send_next_ep0_packet();
}

fn send_next_ep0_packet() {
    let state = unsafe { &mut *STATE.0.get() };
    let remain = state.tx_len.saturating_sub(state.tx_pos);
    let len = remain.min(EP_MAX_PACKET);
    let data = if len == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(state.tx_data.add(state.tx_pos), len) }
    };
    state.tx_pos += len;
    write_ep_tx(EP0, EP0_TX_ADDR, data);
    unsafe {
        set_tx_stat(EP0, EP_STAT_VALID);
        set_rx_stat(EP0, EP_STAT_VALID);
    }
}

fn status_in() {
    let state = unsafe { &mut *STATE.0.get() };
    state.tx_data = core::ptr::null();
    state.tx_len = 0;
    state.tx_pos = 0;
    state.tx_zlp = false;
    write_ep_tx(EP0, EP0_TX_ADDR, &[]);
    unsafe {
        set_tx_stat(EP0, EP_STAT_VALID);
        set_rx_stat(EP0, EP_STAT_VALID);
    }
}

fn stall_ep0() {
    unsafe {
        set_tx_stat(EP0, EP_STAT_STALL);
        set_rx_stat(EP0, EP_STAT_STALL);
    }
}

fn write_ep_tx(ep: usize, addr: u16, data: &[u8]) {
    pma_write(addr, data);
    unsafe {
        write_pma_u16((ep * 8 + 2) as u16, data.len() as u16);
    }
}

unsafe fn set_btable(ep: usize, tx_addr: u16, tx_count: u16, rx_addr: u16, rx_count: u16) {
    let base = (ep * 8) as u16;
    unsafe {
        write_pma_u16(base, tx_addr);
        write_pma_u16(base + 2, tx_count);
        write_pma_u16(base + 4, rx_addr);
        write_pma_u16(base + 6, rx_count);
    }
}

fn rx_count(size: usize) -> u16 {
    if size <= 62 {
        (((size as u16 + 1) / 2) << 10) & 0x7c00
    } else {
        0x8000 | ((((size as u16 + 31) / 32) - 1) << 10)
    }
}

fn ep_reg(ep: usize) -> u16 {
    unsafe { read16((USB_BASE + ep * 4) as *mut u32) }
}

unsafe fn set_ep_reg(ep: usize, base: u16, tx_stat: u16, rx_stat: u16) {
    unsafe {
        write16((USB_BASE + ep * 4) as *mut u32, base);
        set_tx_stat(ep, tx_stat);
        set_rx_stat(ep, rx_stat);
    }
}

unsafe fn set_tx_stat(ep: usize, stat: u16) {
    unsafe {
        let reg = ep_reg(ep);
        let value = (reg & (EP_CTR_RX | EP_CTR_TX | EP_TYPE | EP_KIND | EP_ADDR))
            | ((reg & EP_STAT_TX) ^ (stat << 4));
        write16((USB_BASE + ep * 4) as *mut u32, value);
    }
}

unsafe fn set_rx_stat(ep: usize, stat: u16) {
    unsafe {
        let reg = ep_reg(ep);
        let value = (reg & (EP_CTR_RX | EP_CTR_TX | EP_TYPE | EP_KIND | EP_ADDR))
            | ((reg & EP_STAT_RX) ^ (stat << 12));
        write16((USB_BASE + ep * 4) as *mut u32, value);
    }
}

unsafe fn clear_ctr(ep: usize, rx: bool, tx: bool) {
    unsafe {
        let mut reg = ep_reg(ep);
        reg &= !(EP_DTOG_RX | EP_DTOG_TX | EP_STAT_RX | EP_STAT_TX);
        if rx {
            reg &= !EP_CTR_RX;
        }
        if tx {
            reg &= !EP_CTR_TX;
        }
        write16((USB_BASE + ep * 4) as *mut u32, reg);
    }
}

#[unsafe(export_name = "USB_LP_CAN1_RX0")]
pub extern "C" fn usb_lp_can_rx0_handler() {
    poll();
}

fn pma_write(addr: u16, data: &[u8]) {
    for (index, chunk) in data.chunks(2).enumerate() {
        let lo = chunk[0] as u16;
        let hi = if chunk.len() > 1 {
            (chunk[1] as u16) << 8
        } else {
            0
        };
        unsafe {
            write_pma_u16(addr + (index as u16) * 2, lo | hi);
        }
    }
}

fn pma_read(addr: u16, data: &mut [u8]) {
    for (index, chunk) in data.chunks_mut(2).enumerate() {
        let word = unsafe { read_pma_u16(addr + (index as u16) * 2) };
        chunk[0] = word as u8;
        if chunk.len() > 1 {
            chunk[1] = (word >> 8) as u8;
        }
    }
}

unsafe fn read_pma_u16(offset: u16) -> u16 {
    unsafe { ptr::read_volatile((USB_PMA + offset as usize * 2) as *const u16) }
}

unsafe fn write_pma_u16(offset: u16, value: u16) {
    unsafe {
        ptr::write_volatile((USB_PMA + offset as usize * 2) as *mut u16, value);
    }
}

unsafe fn read16(reg: *mut u32) -> u16 {
    unsafe { ptr::read_volatile(reg as *const u16) }
}

unsafe fn write16(reg: *mut u32, value: u16) {
    unsafe {
        ptr::write_volatile(reg as *mut u16, value);
    }
}
