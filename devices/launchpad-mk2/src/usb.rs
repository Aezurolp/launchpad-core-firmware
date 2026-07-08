// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cell::UnsafeCell;
use core::ptr;

use embassy_stm32::interrupt::{self, InterruptExt};
use firmware_core::app::{MidiEvent, MidiPort};
use heapless::spsc::{Consumer, Producer, Queue};
use static_cell::StaticCell;

const USB_BASE: usize = 0x4000_5c00;
const USB_PMA: usize = 0x4000_6000;

const USB_CNTR: *mut u32 = (USB_BASE + 0x40) as *mut u32;
const USB_ISTR: *mut u32 = (USB_BASE + 0x44) as *mut u32;
const USB_DADDR: *mut u32 = (USB_BASE + 0x4c) as *mut u32;
const USB_BTABLE: *mut u32 = (USB_BASE + 0x50) as *mut u32;

const RCC_CFGR: *mut u32 = 0x4002_1004 as *mut u32;
const RCC_APB1RSTR: *mut u32 = 0x4002_1010 as *mut u32;
const RCC_APB1ENR: *mut u32 = 0x4002_101c as *mut u32;
const RCC_CFGR_USBPRE: u32 = 1 << 22;
const RCC_APB1RSTR_USBRST: u32 = 1 << 23;
const RCC_APB1ENR_USBEN: u32 = 1 << 23;

const ISTR_CTR: u16 = 1 << 15;
const ISTR_ERR: u16 = 1 << 13;
const ISTR_WKUP: u16 = 1 << 12;
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
const EP_TYPE_BULK: u16 = 0b00 << 9;
const EP_CTR_TX: u16 = 1 << 7;
const EP_DTOG_TX: u16 = 1 << 6;
const EP_STAT_TX: u16 = 0b11 << 4;
const EP_ADDR: u16 = 0x0f;

const EP_STAT_STALL: u16 = 0b01;
const EP_STAT_NAK: u16 = 0b10;
const EP_STAT_VALID: u16 = 0b11;

const EP0: usize = 0;
const EP1: usize = 1;

const EP0_MAX_PACKET: usize = 8;
const EP_MAX_PACKET: usize = 64;
const EP0_RX_ADDR: u16 = 0x40;
const EP0_TX_ADDR: u16 = 0x48;
const EP1_RX_ADDR: u16 = 0x50;
const EP1_TX_ADDR: u16 = 0x90;

const SYSEX_MAX_LEN: usize = 256;
const MIDI_QUEUE_SIZE: usize = 1025;
const SYSEX_QUEUE_SIZE: usize = 3;
const MIDI_TX_QUEUE_SIZE: usize = 513;
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

const DEVICE_DESCRIPTOR: [u8; 18] = [
    18,
    DESC_DEVICE,
    0x00,
    0x02,
    0x00,
    0x00,
    0x00,
    EP0_MAX_PACKET as u8,
    0x35,
    0x12,
    0x69,
    0x00,
    0x01,
    0x00,
    1,
    2,
    3,
    1,
];

const CONFIG_TOTAL_LEN: u16 = 0x85;
const CONFIG_DESCRIPTOR: [u8; CONFIG_TOTAL_LEN as usize] = [
    0x09, 0x02, 0x85, 0x00, 0x02, 0x01, 0x00, 0x80, 0xfa, 0x09, 0x04, 0x00, 0x00, 0x00, 0x01, 0x01,
    0x00, 0x02, 0x09, 0x24, 0x01, 0x00, 0x01, 0x09, 0x00, 0x01, 0x01, 0x09, 0x04, 0x01, 0x00, 0x02,
    0x01, 0x03, 0x00, 0x02, 0x07, 0x24, 0x01, 0x00, 0x01, 0x43, 0x00, 0x06, 0x24, 0x02, 0x01, 0x01,
    0x04, 0x06, 0x24, 0x02, 0x02, 0x02, 0x04, 0x09, 0x24, 0x03, 0x01, 0x09, 0x01, 0x02, 0x01, 0x04,
    0x09, 0x24, 0x03, 0x02, 0x0a, 0x01, 0x01, 0x01, 0x04, 0x06, 0x24, 0x02, 0x01, 0x03, 0x05, 0x06,
    0x24, 0x02, 0x02, 0x04, 0x05, 0x09, 0x24, 0x03, 0x01, 0x0b, 0x01, 0x04, 0x01, 0x05, 0x09, 0x24,
    0x03, 0x02, 0x0c, 0x01, 0x03, 0x01, 0x05, 0x09, 0x05, 0x01, 0x02, 0x40, 0x00, 0x00, 0x00, 0x00,
    0x06, 0x25, 0x01, 0x02, 0x01, 0x03, 0x09, 0x05, 0x81, 0x02, 0x40, 0x00, 0x00, 0x00, 0x00, 0x06,
    0x25, 0x01, 0x02, 0x09, 0x0b,
];

const STRING_ZERO: [u8; 4] = [4, DESC_STRING, 0x09, 0x04];
const STRING_MANUFACTURER: [u8; 38] = utf16_string_18(*b"Focusrite A.E. Ltd");
const STRING_PRODUCT: [u8; 28] = utf16_string_13(*b"Launchpad MK2");
const STRING_SERIAL: [u8; 22] = utf16_string_10(*b"COREFW-MK2");
const STRING_PORT1: [u8; 20] = utf16_string_9(*b"MK2 (DAW)");
const STRING_PORT2: [u8; 22] = utf16_string_10(*b"MK2 (MIDI)");

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

const fn utf16_string_13(value: [u8; 13]) -> [u8; 28] {
    let mut out = [0u8; 28];
    out[0] = 28;
    out[1] = DESC_STRING;
    let mut i = 0;
    while i < 13 {
        out[2 + i * 2] = value[i];
        i += 1;
    }
    out
}

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

pub struct SysexMessage {
    pub port: MidiPort,
    pub len: usize,
    pub data: [u8; SYSEX_MAX_LEN],
}

#[derive(Copy, Clone)]
struct UsbMidiPacket {
    data: [u8; 4],
}

struct UsbState {
    pending_address: u8,
    configuration: u8,
    control_buf: [u8; 2],
    tx_data: *const u8,
    tx_len: usize,
    tx_pos: usize,
    tx_zlp: bool,
    pending_tx: Option<UsbMidiPacket>,
    sysex_buf: [u8; SYSEX_MAX_LEN],
    sysex_len: usize,
    sysex_port: MidiPort,
}

impl UsbState {
    const fn new() -> Self {
        Self {
            pending_address: 0,
            configuration: 0,
            control_buf: [0; 2],
            tx_data: core::ptr::null(),
            tx_len: 0,
            tx_pos: 0,
            tx_zlp: false,
            pending_tx: None,
            sysex_buf: [0; SYSEX_MAX_LEN],
            sysex_len: 0,
            sysex_port: MidiPort::Daw,
        }
    }
}

struct StateCell(UnsafeCell<UsbState>);

unsafe impl Sync for StateCell {}

static STATE: StateCell = StateCell(UnsafeCell::new(UsbState::new()));

struct HandleSlot<T> {
    inner: UnsafeCell<Option<T>>,
}

unsafe impl<T> Sync for HandleSlot<T> {}

impl<T> HandleSlot<T> {
    const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(None),
        }
    }

    fn is_empty(&self) -> bool {
        unsafe { (*self.inner.get()).is_none() }
    }

    fn init(&self, value: T) {
        unsafe {
            *self.inner.get() = Some(value);
        }
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        unsafe { (*self.inner.get()).as_mut().map(f) }
    }
}

static MIDI_QUEUE: StaticCell<Queue<MidiEvent, MIDI_QUEUE_SIZE>> = StaticCell::new();
static SYSEX_QUEUE: StaticCell<Queue<SysexMessage, SYSEX_QUEUE_SIZE>> = StaticCell::new();
static MIDI_TX_QUEUE: StaticCell<Queue<UsbMidiPacket, MIDI_TX_QUEUE_SIZE>> = StaticCell::new();

static MIDI_PRODUCER: HandleSlot<Producer<'static, MidiEvent>> = HandleSlot::new();
static MIDI_CONSUMER: HandleSlot<Consumer<'static, MidiEvent>> = HandleSlot::new();
static SYSEX_PRODUCER: HandleSlot<Producer<'static, SysexMessage>> = HandleSlot::new();
static SYSEX_CONSUMER: HandleSlot<Consumer<'static, SysexMessage>> = HandleSlot::new();
static MIDI_TX_PRODUCER: HandleSlot<Producer<'static, UsbMidiPacket>> = HandleSlot::new();
static MIDI_TX_CONSUMER: HandleSlot<Consumer<'static, UsbMidiPacket>> = HandleSlot::new();

pub fn init_event_queues() {
    if MIDI_PRODUCER.is_empty() {
        let midi_queue = MIDI_QUEUE.init(Queue::new());
        let (producer, consumer) = midi_queue.split();
        MIDI_PRODUCER.init(producer);
        MIDI_CONSUMER.init(consumer);
    }

    if SYSEX_PRODUCER.is_empty() {
        let sysex_queue = SYSEX_QUEUE.init(Queue::new());
        let (producer, consumer) = sysex_queue.split();
        SYSEX_PRODUCER.init(producer);
        SYSEX_CONSUMER.init(consumer);
    }

    if MIDI_TX_PRODUCER.is_empty() {
        let midi_tx_queue = MIDI_TX_QUEUE.init(Queue::new());
        let (producer, consumer) = midi_tx_queue.split();
        MIDI_TX_PRODUCER.init(producer);
        MIDI_TX_CONSUMER.init(consumer);
    }
}

pub fn init() {
    unsafe {
        write32(RCC_CFGR, read32(RCC_CFGR) | RCC_CFGR_USBPRE);
        write32(RCC_APB1ENR, read32(RCC_APB1ENR) | RCC_APB1ENR_USBEN);
        write32(RCC_APB1RSTR, read32(RCC_APB1RSTR) | RCC_APB1RSTR_USBRST);
        for _ in 0..16 {
            cortex_m::asm::nop();
        }
        write32(RCC_APB1RSTR, read32(RCC_APB1RSTR) & !RCC_APB1RSTR_USBRST);

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

        use embassy_stm32::interrupt::{Priority, USB_LP_CAN1_RX0};
        USB_LP_CAN1_RX0.unpend();
        
        USB_LP_CAN1_RX0.set_priority(Priority::P2);
        USB_LP_CAN1_RX0.enable();
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
            continue;
        }

        if istr & (ISTR_ERR | ISTR_WKUP | ISTR_SUSP) != 0 {
            unsafe {
                write16(USB_ISTR, !(istr & (ISTR_ERR | ISTR_WKUP | ISTR_SUSP)));
            }
            continue;
        }

        if istr & ISTR_CTR == 0 {
            break;
        }

        match (istr & ISTR_EP_ID) as usize {
            EP0 => poll_ep0(),
            EP1 => poll_ep1(),
            ep => unsafe {
                clear_ctr(ep, false, false);
            },
        }
    }
}

pub fn dequeue_midi_event() -> Option<MidiEvent> {
    MIDI_CONSUMER
        .with_mut(|consumer| consumer.dequeue())
        .flatten()
}

pub fn dequeue_sysex_message() -> Option<SysexMessage> {
    SYSEX_CONSUMER
        .with_mut(|consumer| consumer.dequeue())
        .flatten()
}

pub fn enqueue_tx_message(port: u8, data: &[u8]) -> Result<(), ()> {
    let mut packets = [UsbMidiPacket { data: [0; 4] }; MIDI_TX_MAX_PACKET_COUNT];
    let packet_count = encode_usb_midi_packets(port, data, &mut packets)?;

    MIDI_TX_PRODUCER
        .with_mut(|producer| {
            let free = producer.capacity().saturating_sub(producer.len());
            if free < packet_count {
                return Err(());
            }

            for packet in packets.iter().take(packet_count) {
                producer.enqueue(*packet).map_err(|_| ())?;
            }

            Ok(())
        })
        .ok_or(())??;

    try_start_in_tx();
    Ok(())
}

unsafe fn reset_bus() {
    let state = unsafe { &mut *STATE.0.get() };
    *state = UsbState::new();

    unsafe {
        write16(USB_DADDR, DADDR_EF);
        write16(USB_BTABLE, 0);

        set_btable(EP0, EP0_TX_ADDR, 0, EP0_RX_ADDR, rx_count(EP0_MAX_PACKET));
        set_btable(EP1, EP1_TX_ADDR, 0, EP1_RX_ADDR, rx_count(EP_MAX_PACKET));

        set_ep_reg(
            EP0,
            EP_TYPE_CONTROL | EP0 as u16,
            EP_STAT_NAK,
            EP_STAT_VALID,
        );
        set_ep_reg(EP1, EP_TYPE_BULK | EP1 as u16, EP_STAT_NAK, EP_STAT_NAK);
    }
}

fn poll_ep0() {
    let reg = ep_reg(EP0);

    if reg & EP_CTR_RX != 0 {
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
    }

    if ep_reg(EP0) & EP_CTR_TX != 0 {
        unsafe {
            clear_ctr(EP0, false, true);
        }

        let state = unsafe { &mut *STATE.0.get() };
        if state.pending_address != 0 {
            unsafe {
                write16(USB_DADDR, DADDR_EF | state.pending_address as u16);
            }
            state.pending_address = 0;
        }

        if state.tx_pos < state.tx_len {
            send_next_ep0_packet();
        } else if state.tx_zlp {
            state.tx_zlp = false;
            write_ep_tx(EP0, EP0_TX_ADDR, &[]);
            unsafe {
                set_tx_stat(EP0, EP_STAT_VALID);
            }
        } else {
            unsafe {
                set_tx_stat(EP0, EP_STAT_NAK);
                set_rx_stat(EP0, EP_STAT_VALID);
            }
        }
    }
}

fn poll_ep1() {
    let reg = ep_reg(EP1);

    if reg & EP_CTR_RX != 0 {
        let len = unsafe { read_pma_u16((EP1 * 8 + 6) as u16) & 0x03ff } as usize;
        let mut buf = [0u8; EP_MAX_PACKET];
        let len = len.min(buf.len());
        pma_read(EP1_RX_ADDR, &mut buf[..len]);

        for packet in buf[..len].chunks_exact(4) {
            handle_usb_midi_packet(packet);
        }

        unsafe {
            clear_ctr(EP1, true, false);
            write_pma_u16((EP1 * 8 + 6) as u16, rx_count(EP_MAX_PACKET));
            set_rx_stat(EP1, EP_STAT_VALID);
        }
    }

    if ep_reg(EP1) & EP_CTR_TX != 0 {
        unsafe {
            clear_ctr(EP1, false, true);
            set_tx_stat(EP1, EP_STAT_NAK);
        }
        try_start_in_tx();
    }
}

fn handle_usb_midi_packet(packet: &[u8]) {
    let cin = packet[0] & 0x0f;
    let Some(port) = MidiPort::from_usb_cable(packet[0] >> 4) else {
        return;
    };

    match cin {
        0x4 => {
            let state = unsafe { &mut *STATE.0.get() };
            state.sysex_port = port;
            append_sysex_bytes(state, &packet[1..4]);
        }
        0x5 => {
            let state = unsafe { &mut *STATE.0.get() };
            state.sysex_port = port;
            append_sysex_bytes(state, &packet[1..2]);
            flush_sysex(state);
        }
        0x6 => {
            let state = unsafe { &mut *STATE.0.get() };
            state.sysex_port = port;
            append_sysex_bytes(state, &packet[1..3]);
            flush_sysex(state);
        }
        0x7 => {
            let state = unsafe { &mut *STATE.0.get() };
            state.sysex_port = port;
            append_sysex_bytes(state, &packet[1..4]);
            flush_sysex(state);
        }
        0x8 | 0x9 | 0xA | 0xB | 0xC | 0xD | 0xE => {
            let event = MidiEvent {
                port,
                status: packet[1],
                data1: packet[2],
                data2: packet[3],
            };
            let _ = MIDI_PRODUCER.with_mut(|producer| producer.enqueue(event));
        }
        0xF => {
            let event = MidiEvent {
                port,
                status: packet[1],
                data1: 0,
                data2: 0,
            };
            let _ = MIDI_PRODUCER.with_mut(|producer| producer.enqueue(event));
        }
        _ => {}
    }
}

fn append_sysex_bytes(state: &mut UsbState, bytes: &[u8]) {
    let space = SYSEX_MAX_LEN - state.sysex_len;
    let to_copy = bytes.len().min(space);
    state.sysex_buf[state.sysex_len..state.sysex_len + to_copy].copy_from_slice(&bytes[..to_copy]);
    state.sysex_len += to_copy;
}

fn flush_sysex(state: &mut UsbState) {
    if state.sysex_len == 0 {
        return;
    }

    let mut data = [0u8; SYSEX_MAX_LEN];
    data[..state.sysex_len].copy_from_slice(&state.sysex_buf[..state.sysex_len]);

    let _ = SYSEX_PRODUCER.with_mut(|producer| {
        producer.enqueue(SysexMessage {
            port: state.sysex_port,
            len: state.sysex_len,
            data,
        })
    });
    state.sysex_len = 0;
}

fn try_start_in_tx() {
    let state = unsafe { &mut *STATE.0.get() };
    if state.configuration == 0 || ep_reg(EP1) & EP_STAT_TX != (EP_STAT_NAK << 4) {
        return;
    }

    let Some(packet) = state.pending_tx.take().or_else(|| {
        MIDI_TX_CONSUMER
            .with_mut(|consumer| consumer.dequeue())
            .flatten()
    }) else {
        return;
    };

    let mut buf = [0u8; EP_MAX_PACKET];
    buf[..4].copy_from_slice(&packet.data);
    let mut len = 4usize;

    while len + 4 <= buf.len() {
        let Some(packet) = MIDI_TX_CONSUMER
            .with_mut(|consumer| consumer.dequeue())
            .flatten()
        else {
            break;
        };
        buf[len..len + 4].copy_from_slice(&packet.data);
        len += 4;
    }

    write_ep_tx(EP1, EP1_TX_ADDR, &buf[..len]);
    unsafe {
        set_tx_stat(EP1, EP_STAT_VALID);
    }
}

fn encode_usb_midi_packets(
    port: u8,
    data: &[u8],
    out: &mut [UsbMidiPacket; MIDI_TX_MAX_PACKET_COUNT],
) -> Result<usize, ()> {
    if data.is_empty() {
        return Err(());
    }

    let cable = (port & 0x0f) << 4;

    if data[0] == 0xf0 {
        return encode_sysex_packets(cable, data, out);
    }

    let Some((cin, message_len)) = short_message_format(data[0]) else {
        return Err(());
    };

    if data.len() < message_len {
        return Err(());
    }

    out[0] = UsbMidiPacket {
        data: [
            cable | cin,
            data[0],
            if message_len > 1 { data[1] } else { 0 },
            if message_len > 2 { data[2] } else { 0 },
        ],
    };
    Ok(1)
}

fn encode_sysex_packets(
    cable: u8,
    data: &[u8],
    out: &mut [UsbMidiPacket; MIDI_TX_MAX_PACKET_COUNT],
) -> Result<usize, ()> {
    let mut src = 0usize;
    let mut dst = 0usize;

    while src < data.len() {
        if dst >= out.len() {
            return Err(());
        }

        let remain = data.len() - src;
        let take = remain.min(3);
        let cin = if remain > 3 {
            0x4
        } else {
            match remain {
                1 => 0x5,
                2 => 0x6,
                3 => 0x7,
                _ => return Err(()),
            }
        };

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
                    set_tx_stat(EP1, EP_STAT_NAK);
                    set_rx_stat(EP1, EP_STAT_VALID);
                }
                try_start_in_tx();
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
            state.control_buf = [0, 0];
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
        (DESC_STRING, 3) => Some(&STRING_SERIAL),
        (DESC_STRING, 4) => Some(&STRING_PORT1),
        (DESC_STRING, 5) => Some(&STRING_PORT2),
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
        state.tx_len != 0 && state.tx_len % EP0_MAX_PACKET == 0 && requested_len > state.tx_len;
    send_next_ep0_packet();
}

fn send_next_ep0_packet() {
    let state = unsafe { &mut *STATE.0.get() };
    let remain = state.tx_len.saturating_sub(state.tx_pos);
    let len = remain.min(EP0_MAX_PACKET);
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

#[embassy_stm32::interrupt]
unsafe fn USB_LP_CAN1_RX0() {
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

unsafe fn read32(reg: *mut u32) -> u32 {
    unsafe { ptr::read_volatile(reg) }
}

unsafe fn write32(reg: *mut u32, value: u32) {
    unsafe {
        ptr::write_volatile(reg, value);
    }
}
