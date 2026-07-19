// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cell::UnsafeCell;

use embassy_usb::driver::{Driver as UsbDriver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::types::StringIndex;
use embassy_usb::{Builder, Config, Handler};
use esp_hal::otg_fs::Usb;
use esp_hal::otg_fs::asynch::{Config as OtgConfig, Driver};
use firmware_core::app::{MidiEvent, MidiPort};
use heapless::spsc::{Consumer, Producer, Queue};
use static_cell::StaticCell;

const SYSEX_MAX_LEN: usize = 256;
const MIDI_QUEUE_SIZE: usize = 1025;
const SYSEX_QUEUE_SIZE: usize = 5;
const MIDI_TX_MAX_LEN: usize = 256;
const MIDI_TX_QUEUE_SIZE: usize = 17;

static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static MIDI_NAME_HANDLER: StaticCell<NamedStringsHandler> = StaticCell::new();
static MIDI_QUEUE: StaticCell<Queue<MidiEvent, MIDI_QUEUE_SIZE>> = StaticCell::new();
static SYSEX_QUEUE: StaticCell<Queue<SysexMessage, SYSEX_QUEUE_SIZE>> = StaticCell::new();
static MIDI_TX_QUEUE: StaticCell<Queue<MidiTxMessage, MIDI_TX_QUEUE_SIZE>> = StaticCell::new();

pub struct SysexMessage {
    pub port: MidiPort,
    pub len: usize,
    pub data: [u8; SYSEX_MAX_LEN],
}

pub struct MidiTxMessage {
    len: usize,
    data: [u8; MIDI_TX_MAX_LEN],
}

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
        unsafe { *self.inner.get() = Some(value) }
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        unsafe { (*self.inner.get()).as_mut().map(f) }
    }
}

static TX_SIGNAL: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    (),
> = embassy_sync::signal::Signal::new();
static MIDI_PRODUCER: HandleSlot<Producer<'static, MidiEvent>> = HandleSlot::new();
static MIDI_CONSUMER: HandleSlot<Consumer<'static, MidiEvent>> = HandleSlot::new();
static SYSEX_PRODUCER: HandleSlot<Producer<'static, SysexMessage>> = HandleSlot::new();
static SYSEX_CONSUMER: HandleSlot<Consumer<'static, SysexMessage>> = HandleSlot::new();
static MIDI_TX_PRODUCER: HandleSlot<Producer<'static, MidiTxMessage>> = HandleSlot::new();
static MIDI_TX_CONSUMER: HandleSlot<Consumer<'static, MidiTxMessage>> = HandleSlot::new();

pub fn init_event_queues() {
    if MIDI_PRODUCER.is_empty() {
        let (producer, consumer) = MIDI_QUEUE.init(Queue::new()).split();
        MIDI_PRODUCER.init(producer);
        MIDI_CONSUMER.init(consumer);
    }
    if SYSEX_PRODUCER.is_empty() {
        let (producer, consumer) = SYSEX_QUEUE.init(Queue::new()).split();
        SYSEX_PRODUCER.init(producer);
        SYSEX_CONSUMER.init(consumer);
    }
    if MIDI_TX_PRODUCER.is_empty() {
        let (producer, consumer) = MIDI_TX_QUEUE.init(Queue::new()).split();
        MIDI_TX_PRODUCER.init(producer);
        MIDI_TX_CONSUMER.init(consumer);
    }
}

pub fn dequeue_midi_event() -> Option<MidiEvent> {
    MIDI_CONSUMER.with_mut(|queue| queue.dequeue()).flatten()
}

pub fn dequeue_sysex_message() -> Option<SysexMessage> {
    SYSEX_CONSUMER.with_mut(|queue| queue.dequeue()).flatten()
}

pub fn enqueue_tx_message(data: &[u8]) {
    if data.is_empty() || data.len() > MIDI_TX_MAX_LEN {
        return;
    }
    let mut message = MidiTxMessage {
        len: data.len(),
        data: [0; MIDI_TX_MAX_LEN],
    };
    message.data[..data.len()].copy_from_slice(data);
    if MIDI_TX_PRODUCER
        .with_mut(|queue| queue.enqueue(message))
        .is_some_and(|result| result.is_ok())
    {
        TX_SIGNAL.signal(());
    }
}

pub fn make_driver(usb: Usb<'static>) -> Driver<'static> {
    Driver::new(usb, EP_OUT_BUFFER.init([0; 256]), OtgConfig::default())
}

pub fn spawn(spawner: &embassy_executor::Spawner, driver: Driver<'static>) {
    spawner.spawn(usb_midi_task(driver).expect("usb_midi_task spawn"));
}

#[embassy_executor::task]
async fn usb_midi_task(driver: Driver<'static>) {
    let mut config = Config::new(0x1235, 0x4051);
    config.device_release = 0x0200;
    config.manufacturer = Some("Focusrite A.E. Ltd");
    config.product = Some("Mystrix Pro");
    config.serial_number = Some("COREFW-MXPRO");
    config.max_power = 500;
    config.max_packet_size_0 = 64;

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );
    let mut midi = MiniMidiClass::new(&mut builder);
    let mut usb = builder.build();
    embassy_futures::join::join(usb.run(), async {
        loop {
            midi.wait_connection().await;
            let _ = midi_io(&mut midi).await;
        }
    })
    .await;
}

struct Disconnected;

impl From<EndpointError> for Disconnected {
    fn from(value: EndpointError) -> Self {
        match value {
            EndpointError::BufferOverflow => panic!("USB MIDI buffer overflow"),
            EndpointError::Disabled => Self,
        }
    }
}

async fn midi_io(class: &mut MiniMidiClass<'static, Driver<'static>>) -> Result<(), Disconnected> {
    let mut buffer = [0u8; 64];
    let mut sysex = [0u8; SYSEX_MAX_LEN];
    let mut sysex_len = 0;
    let mut sysex_port = MidiPort::Midi;
    loop {
        flush_tx_queue(&mut class.write_ep).await?;
        match embassy_futures::select::select(class.read_ep.read(&mut buffer), TX_SIGNAL.wait())
            .await
        {
            embassy_futures::select::Either::First(result) => {
                let count = result.map_err(Disconnected::from)?;
                for packet in buffer[..count].chunks_exact(4) {
                    handle_usb_midi_packet(packet, &mut sysex, &mut sysex_len, &mut sysex_port);
                }
            }
            embassy_futures::select::Either::Second(_) => TX_SIGNAL.reset(),
        }
    }
}

fn handle_usb_midi_packet(
    packet: &[u8],
    sysex: &mut [u8; SYSEX_MAX_LEN],
    length: &mut usize,
    port: &mut MidiPort,
) {
    let cin = packet[0] & 0x0f;
    // Mystrix Pro exposes one USB-MIDI cable only; all traffic belongs to MIDI.
    let midi_port = MidiPort::Midi;
    match cin {
        0x4 => {
            *port = midi_port;
            append_sysex(sysex, length, &packet[1..4]);
        }
        0x5..=0x7 => {
            *port = midi_port;
            append_sysex(sysex, length, &packet[1..(cin - 3) as usize]);
            flush_sysex(sysex, length, *port);
        }
        0x8..=0xE => {
            let _ = enqueue_midi(MidiEvent {
                port: midi_port,
                status: packet[1],
                data1: packet[2],
                data2: packet[3],
            });
        }
        _ => {}
    }
}

fn append_sysex(buffer: &mut [u8; SYSEX_MAX_LEN], length: &mut usize, bytes: &[u8]) {
    for byte in bytes {
        if *length < SYSEX_MAX_LEN {
            buffer[*length] = *byte;
            *length += 1;
        }
    }
}

fn flush_sysex(buffer: &mut [u8; SYSEX_MAX_LEN], length: &mut usize, port: MidiPort) {
    if *length == 0 {
        return;
    }
    let mut data = [0; SYSEX_MAX_LEN];
    data[..*length].copy_from_slice(&buffer[..*length]);
    let _ = SYSEX_PRODUCER.with_mut(|queue| {
        queue.enqueue(SysexMessage {
            port,
            len: *length,
            data,
        })
    });
    *length = 0;
}

fn enqueue_midi(event: MidiEvent) -> Result<(), MidiEvent> {
    let mut event = Some(event);
    MIDI_PRODUCER
        .with_mut(|queue| queue.enqueue(event.take().unwrap()))
        .unwrap_or_else(|| Err(event.unwrap()))
}

async fn flush_tx_queue(write_ep: &mut impl EndpointIn) -> Result<(), Disconnected> {
    while let Some(message) = MIDI_TX_CONSUMER.with_mut(|queue| queue.dequeue()).flatten() {
        let mut packets = [0; 64];
        let length = encode_packets(&message.data[..message.len], &mut packets);
        if length != 0 {
            write_ep
                .write(&packets[..length])
                .await
                .map_err(Disconnected::from)?;
        }
    }
    Ok(())
}

fn encode_packets(data: &[u8], output: &mut [u8; 64]) -> usize {
    if data.is_empty() {
        return 0;
    }
    let cable = 0;
    if data[0] == 0xf0 {
        return encode_sysex(cable, data, output);
    }
    let Some((cin, length)) = short_message_format(data[0]) else {
        return 0;
    };
    if data.len() < length {
        return 0;
    }
    output[..4].copy_from_slice(&[
        cable | cin,
        data[0],
        *data.get(1).unwrap_or(&0),
        *data.get(2).unwrap_or(&0),
    ]);
    4
}

fn encode_sysex(cable: u8, data: &[u8], output: &mut [u8; 64]) -> usize {
    let mut source = 0;
    let mut destination = 0;
    while source < data.len() && destination + 4 <= output.len() {
        let remaining = data.len() - source;
        let bytes = remaining.min(3);
        output[destination] = cable
            | if remaining > 3 {
                0x4
            } else {
                0x4 + bytes as u8
            };
        output[destination + 1] = data[source];
        output[destination + 2] = if bytes > 1 { data[source + 1] } else { 0 };
        output[destination + 3] = if bytes > 2 { data[source + 2] } else { 0 };
        source += bytes;
        destination += 4;
    }
    destination
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

struct NamedStringsHandler {
    interface: StringIndex,
    jack: StringIndex,
}

impl Handler for NamedStringsHandler {
    fn get_string(&mut self, index: StringIndex, _language: u16) -> Option<&str> {
        if index == self.interface {
            Some("MXPRO")
        } else if index == self.jack {
            Some("MXPRO (MIDI)")
        } else {
            None
        }
    }
}

struct MiniMidiClass<'d, D: UsbDriver<'d>> {
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
}

impl<'d, D: UsbDriver<'d>> MiniMidiClass<'d, D> {
    fn new(builder: &mut Builder<'d, D>) -> Self {
        let interface_string = builder.string();
        let jack_string = builder.string();
        builder.handler(MIDI_NAME_HANDLER.init(NamedStringsHandler {
            interface: interface_string,
            jack: jack_string,
        }));
        let mut function = builder.function(0x01, 0x01, 0x00);
        let mut control = function.interface();
        let audio_interface = control.interface_number();
        let mut control_alt = control.alt_setting(0x01, 0x01, 0x00, None);
        control_alt.descriptor(
            0x24,
            &[
                0x01,
                0x00,
                0x01,
                0x09,
                0x00,
                0x01,
                u8::from(audio_interface) + 1,
            ],
        );

        let mut streaming = function.interface();
        let mut alt = streaming.alt_setting(0x01, 0x03, 0x00, Some(interface_string));
        // MIDI-streaming class-specific descriptor total: 37 bytes.
        alt.descriptor(0x24, &[0x01, 0x00, 0x01, 0x25, 0x00]);
        // One virtual USB-MIDI cable: external IN, embedded OUT, external OUT, embedded IN.
        alt.descriptor(0x24, &[0x02, 0x02, 0x01, 0x00]);
        alt.descriptor(0x24, &[0x02, 0x01, 0x02, jack_string.into()]);
        alt.descriptor(0x24, &[0x03, 0x02, 0x03, 0x01, 0x02, 0x01, 0x00]);
        alt.descriptor(
            0x24,
            &[0x03, 0x01, 0x04, 0x01, 0x01, 0x01, jack_string.into()],
        );
        let read_ep = alt.endpoint_bulk_out(None, 64);
        alt.descriptor(0x25, &[0x01, 0x01, 0x02]);
        let write_ep = alt.endpoint_bulk_in(None, 64);
        alt.descriptor(0x25, &[0x01, 0x01, 0x04]);
        Self { read_ep, write_ep }
    }

    async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }
}
