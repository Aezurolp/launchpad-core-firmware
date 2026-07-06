use core::cell::UnsafeCell;

use embassy_executor::Spawner;
use embassy_stm32::Peri;
use embassy_stm32::bind_interrupts;
use embassy_stm32::peripherals;
use embassy_stm32::usb::{self, Driver};
use embassy_time::Timer;
use embassy_usb::driver::{Driver as UsbDriver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::types::StringIndex;
use embassy_usb::{Builder, Config, Handler};
use firmware_core::app::MidiEvent;
use firmware_core::app::MidiPort;
use heapless::spsc::{Consumer, Producer, Queue};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static MIDI_NAME_HANDLER: StaticCell<NamedStringsHandler> = StaticCell::new();
static MIDI_QUEUE: StaticCell<Queue<MidiEvent, MIDI_QUEUE_SIZE>> = StaticCell::new();
static SYSEX_QUEUE: StaticCell<Queue<SysexMessage, SYSEX_QUEUE_SIZE>> = StaticCell::new();

const SYSEX_MAX_LEN: usize = 600;
const MIDI_QUEUE_SIZE: usize = 1025;
const SYSEX_QUEUE_SIZE: usize = 5;
const MIDI_TX_MAX_LEN: usize = 600;
const MIDI_TX_QUEUE_SIZE: usize = 17;

pub struct SysexMessage {
    pub port: MidiPort,
    pub len: usize,
    pub data: [u8; SYSEX_MAX_LEN],
}

pub struct MidiTxMessage {
    pub port: u8,
    pub len: usize,
    pub data: [u8; MIDI_TX_MAX_LEN],
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
        unsafe {
            *self.inner.get() = Some(value);
        }
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
static MIDI_TX_QUEUE: StaticCell<Queue<MidiTxMessage, MIDI_TX_QUEUE_SIZE>> = StaticCell::new();
static MIDI_TX_PRODUCER: HandleSlot<Producer<'static, MidiTxMessage>> = HandleSlot::new();
static MIDI_TX_CONSUMER: HandleSlot<Consumer<'static, MidiTxMessage>> = HandleSlot::new();

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
    if data.is_empty() || data.len() > MIDI_TX_MAX_LEN {
        return Err(());
    }

    let mut message = MidiTxMessage {
        port,
        len: data.len(),
        data: [0; MIDI_TX_MAX_LEN],
    };
    message.data[..data.len()].copy_from_slice(data);

    let res = MIDI_TX_PRODUCER
        .with_mut(|producer| producer.enqueue(message))
        .ok_or(())?
        .map_err(|_| ());

    if res.is_ok() {
        TX_SIGNAL.signal(());
    }

    res
}

pub fn make_driver(
    usb_otg_fs: Peri<'static, peripherals::USB_OTG_FS>,
    pa12: Peri<'static, peripherals::PA12>,
    pa11: Peri<'static, peripherals::PA11>,
) -> Driver<'static, peripherals::USB_OTG_FS> {
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);

    let mut config = usb::Config::default();
    config.vbus_detection = false;

    Driver::new_fs(usb_otg_fs, Irqs, pa12, pa11, ep_out_buffer, config)
}

pub fn spawn(spawner: &Spawner, driver: Driver<'static, peripherals::USB_OTG_FS>) {
    spawner.spawn(usb_midi_task(driver).expect("usb_midi_task spawn"));
}

#[embassy_executor::task]
async fn usb_midi_task(driver: Driver<'static, peripherals::USB_OTG_FS>) {
    let mut config = Config::new(0x1235, 0x0123);

    config.device_release = 0x0200;
    config.manufacturer = Some("Focusrite - Novation");
    config.product = Some("Launchpad Pro MK3");
    config.serial_number = Some("COREFW-LPPMK3");
    config.max_power = 500;
    config.max_packet_size_0 = 64;

    let config_descriptor = CONFIG_DESCRIPTOR.init([0; 256]);
    let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 64]);

    let mut builder = Builder::new(
        driver,
        config,
        config_descriptor,
        bos_descriptor,
        &mut [],
        control_buf,
    );

    let mut midi = MiniMidiClass::new(
        &mut builder,
        2,
        2,
        64,
        "Launchpad Pro MK3",
        ["PRO MK3 (DAW)", "PRO MK3 (MIDI)"],
    );
    let mut usb = builder.build();

    let usb_fut = usb.run();
    let midi_fut = async {
        // Wait for the first poll of usb.run() to trigger Bus::init() and RCC reset
        Timer::after_millis(50).await;
        crate::init_usb_board();

        loop {
            midi.wait_connection().await;
            let _ = midi_io(&mut midi).await;
        }
    };

    embassy_futures::join::join(usb_fut, midi_fut).await;
}

struct Disconnected;

impl From<EndpointError> for Disconnected {
    fn from(value: EndpointError) -> Self {
        match value {
            EndpointError::BufferOverflow => panic!("USB MIDI buffer overflow"),
            EndpointError::Disabled => Disconnected,
        }
    }
}

async fn midi_io(
    class: &mut MiniMidiClass<'static, Driver<'static, peripherals::USB_OTG_FS>>,
) -> Result<(), Disconnected> {
    let mut buf = [0u8; 64];
    let mut sysex_buf = [0u8; SYSEX_MAX_LEN];
    let mut sysex_len = 0usize;
    let mut sysex_port = MidiPort::Daw;

    let read_ep = &mut class.read_ep;
    let write_ep = &mut class.write_ep;

    loop {
        flush_tx_queue(write_ep).await?;

        match embassy_futures::select::select(read_ep.read(&mut buf), TX_SIGNAL.wait()).await {
            embassy_futures::select::Either::First(read_result) => {
                let n = read_result.map_err(|e| Disconnected::from(e))?;
                for packet in buf[..n].chunks_exact(4) {
                    handle_usb_midi_packet(packet, &mut sysex_buf, &mut sysex_len, &mut sysex_port);
                }
            }
            embassy_futures::select::Either::Second(_) => {
                TX_SIGNAL.reset();
            }
        }
    }
}

fn handle_usb_midi_packet(
    packet: &[u8],
    sysex_buf: &mut [u8; SYSEX_MAX_LEN],
    sysex_len: &mut usize,
    sysex_port: &mut MidiPort,
) {
    let cin = packet[0] & 0x0f;
    let port = MidiPort::from_cable(packet[0] >> 4);

    match cin {
        0x4 => {
            *sysex_port = port;
            append_sysex_bytes(sysex_buf, sysex_len, &packet[1..4]);
        }
        0x5 => {
            *sysex_port = port;
            append_sysex_bytes(sysex_buf, sysex_len, &packet[1..2]);
            flush_sysex(sysex_buf, sysex_len, *sysex_port);
        }
        0x6 => {
            *sysex_port = port;
            append_sysex_bytes(sysex_buf, sysex_len, &packet[1..3]);
            flush_sysex(sysex_buf, sysex_len, *sysex_port);
        }
        0x7 => {
            *sysex_port = port;
            append_sysex_bytes(sysex_buf, sysex_len, &packet[1..4]);
            flush_sysex(sysex_buf, sysex_len, *sysex_port);
        }
        _ => {
            if packet[1] != 0 {
                let _ = enqueue_midi_event(MidiEvent {
                    port,
                    status: packet[1],
                    data1: packet[2],
                    data2: packet[3],
                });
            }
        }
    }
}

fn append_sysex_bytes(sysex_buf: &mut [u8; SYSEX_MAX_LEN], sysex_len: &mut usize, bytes: &[u8]) {
    for &byte in bytes {
        if *sysex_len < SYSEX_MAX_LEN {
            sysex_buf[*sysex_len] = byte;
            *sysex_len += 1;
        }
    }
}

fn flush_sysex(sysex_buf: &mut [u8; SYSEX_MAX_LEN], sysex_len: &mut usize, port: MidiPort) {
    if *sysex_len == 0 {
        return;
    }

    let mut data = [0u8; SYSEX_MAX_LEN];
    data[..*sysex_len].copy_from_slice(&sysex_buf[..*sysex_len]);

    let _ = enqueue_sysex_message(SysexMessage {
        port,
        len: *sysex_len,
        data,
    });
    *sysex_len = 0;
}

fn enqueue_midi_event(event: MidiEvent) -> Result<(), MidiEvent> {
    let mut event = Some(event);

    match MIDI_PRODUCER.with_mut(|producer| producer.enqueue(event.take().unwrap())) {
        Some(result) => result,
        None => Err(event.unwrap()),
    }
}

fn enqueue_sysex_message(message: SysexMessage) -> Result<(), SysexMessage> {
    let mut message = Some(message);

    match SYSEX_PRODUCER.with_mut(|producer| producer.enqueue(message.take().unwrap())) {
        Some(result) => result,
        None => Err(message.unwrap()),
    }
}

async fn flush_tx_queue(write_ep: &mut impl EndpointIn) -> Result<(), Disconnected> {
    while let Some(message) = MIDI_TX_CONSUMER
        .with_mut(|consumer| consumer.dequeue())
        .flatten()
    {
        let data = &message.data[..message.len];
        let mut offset = 0usize;
        while offset < data.len() {
            let mut packet_buf = [0u8; 64];
            let (packet_len, consumed) = if data[0] == 0xf0 {
                encode_sysex_packets((message.port & 0x0f) << 4, &data[offset..], &mut packet_buf)
            } else {
                encode_usb_midi_packets(message.port, &data[offset..], &mut packet_buf)
            };
            if packet_len == 0 || consumed == 0 {
                break;
            }
            write_ep
                .write(&packet_buf[..packet_len])
                .await
                .map_err(|e| Disconnected::from(e))?;
            offset += consumed;
        }
    }

    Ok(())
}

fn encode_usb_midi_packets(port: u8, data: &[u8], out: &mut [u8; 64]) -> (usize, usize) {
    if data.is_empty() {
        return (0, 0);
    }

    let cable = (port & 0x0f) << 4;

    if data[0] == 0xF0 {
        return encode_sysex_packets(cable, data, out);
    }

    let Some((cin, message_len)) = short_message_format(data[0]) else {
        return (0, 0);
    };

    if data.len() < message_len {
        return (0, 0);
    }

    out[0] = cable | cin;
    out[1] = data[0];
    out[2] = if message_len > 1 { data[1] } else { 0 };
    out[3] = if message_len > 2 { data[2] } else { 0 };
    (4, message_len)
}

fn encode_sysex_packets(cable: u8, data: &[u8], out: &mut [u8; 64]) -> (usize, usize) {
    let mut src = 0usize;
    let mut dst = 0usize;

    while src < data.len() && dst + 4 <= out.len() {
        let remain = data.len() - src;

        if remain > 3 && dst + 8 <= out.len() {
            out[dst] = cable | 0x4;
            out[dst + 1] = data[src];
            out[dst + 2] = data[src + 1];
            out[dst + 3] = data[src + 2];
            src += 3;
            dst += 4;
            continue;
        }

        if remain > 3 {
            break;
        }

        out[dst] = cable
            | match remain {
                1 => 0x5,
                2 => 0x6,
                3 => 0x7,
                _ => 0x0,
            };
        out[dst + 1] = data[src];
        out[dst + 2] = if remain > 1 { data[src + 1] } else { 0 };
        out[dst + 3] = if remain > 2 { data[src + 2] } else { 0 };
        dst += 4;
        src = data.len();
        break;
    }

    (dst, src)
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
    interface_index: StringIndex,
    interface_value: &'static str,
    jack_indices: [StringIndex; 2],
    jack_values: [&'static str; 2],
}

impl Handler for NamedStringsHandler {
    fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&str> {
        if index == self.interface_index {
            Some(self.interface_value)
        } else if index == self.jack_indices[0] {
            Some(self.jack_values[0])
        } else if index == self.jack_indices[1] {
            Some(self.jack_values[1])
        } else {
            None
        }
    }
}

const USB_AUDIO_CLASS: u8 = 0x01;
const USB_AUDIOCONTROL_SUBCLASS: u8 = 0x01;
const USB_MIDISTREAMING_SUBCLASS: u8 = 0x03;
const MIDI_IN_JACK_SUBTYPE: u8 = 0x02;
const MIDI_OUT_JACK_SUBTYPE: u8 = 0x03;
const EMBEDDED: u8 = 0x01;
const EXTERNAL: u8 = 0x02;
const CS_INTERFACE: u8 = 0x24;
const CS_ENDPOINT: u8 = 0x25;
const HEADER_SUBTYPE: u8 = 0x01;
const MS_HEADER_SUBTYPE: u8 = 0x01;
const MS_GENERAL: u8 = 0x01;
const PROTOCOL_NONE: u8 = 0x00;
const MIDI_IN_SIZE: u8 = 0x06;
const MIDI_OUT_SIZE: u8 = 0x09;

struct MiniMidiClass<'d, D: UsbDriver<'d>> {
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
}

impl<'d, D: UsbDriver<'d>> MiniMidiClass<'d, D> {
    fn new(
        builder: &mut Builder<'d, D>,
        n_in_jacks: u8,
        n_out_jacks: u8,
        max_packet_size: u16,
        interface_name: &'static str,
        jack_names: [&'static str; 2],
    ) -> Self {
        let interface_string_index = builder.string();
        let jack_string_indices = [builder.string(), builder.string()];
        let handler = MIDI_NAME_HANDLER.init(NamedStringsHandler {
            interface_index: interface_string_index,
            interface_value: interface_name,
            jack_indices: jack_string_indices,
            jack_values: jack_names,
        });
        builder.handler(handler);

        let mut func = builder.function(USB_AUDIO_CLASS, USB_AUDIOCONTROL_SUBCLASS, PROTOCOL_NONE);

        let mut iface = func.interface();
        let audio_if = iface.interface_number();
        let midi_if = u8::from(audio_if) + 1;
        let mut alt = iface.alt_setting(
            USB_AUDIO_CLASS,
            USB_AUDIOCONTROL_SUBCLASS,
            PROTOCOL_NONE,
            None,
        );
        alt.descriptor(
            CS_INTERFACE,
            &[HEADER_SUBTYPE, 0x00, 0x01, 0x09, 0x00, 0x01, midi_if],
        );

        let mut iface = func.interface();
        let mut alt = iface.alt_setting(
            USB_AUDIO_CLASS,
            USB_MIDISTREAMING_SUBCLASS,
            PROTOCOL_NONE,
            Some(interface_string_index),
        );

        let midi_streaming_total_length = 7
            + (n_in_jacks + n_out_jacks) as usize * (MIDI_IN_SIZE + MIDI_OUT_SIZE) as usize
            + 7
            + (4 + n_out_jacks as usize)
            + 7
            + (4 + n_in_jacks as usize);

        alt.descriptor(
            CS_INTERFACE,
            &[
                MS_HEADER_SUBTYPE,
                0x00,
                0x01,
                (midi_streaming_total_length & 0xFF) as u8,
                ((midi_streaming_total_length >> 8) & 0xFF) as u8,
            ],
        );

        let in_jack_id_ext = |index| 2 * index + 1;
        let out_jack_id_emb = |index| 2 * index + 2;
        let out_jack_id_ext = |index| 2 * n_in_jacks + 2 * index + 1;
        let in_jack_id_emb = |index| 2 * n_in_jacks + 2 * index + 2;

        for i in 0..n_in_jacks {
            alt.descriptor(
                CS_INTERFACE,
                &[MIDI_IN_JACK_SUBTYPE, EXTERNAL, in_jack_id_ext(i), 0x00],
            );
        }

        for i in 0..n_out_jacks {
            let jack_string_index = jack_string_indices
                .get(i as usize)
                .copied()
                .unwrap_or(interface_string_index);
            alt.descriptor(
                CS_INTERFACE,
                &[
                    MIDI_IN_JACK_SUBTYPE,
                    EMBEDDED,
                    in_jack_id_emb(i),
                    jack_string_index.into(),
                ],
            );
        }

        for i in 0..n_out_jacks {
            alt.descriptor(
                CS_INTERFACE,
                &[
                    MIDI_OUT_JACK_SUBTYPE,
                    EXTERNAL,
                    out_jack_id_ext(i),
                    0x01,
                    in_jack_id_emb(i),
                    0x01,
                    0x00,
                ],
            );
        }

        for i in 0..n_in_jacks {
            let jack_string_index = jack_string_indices
                .get(i as usize)
                .copied()
                .unwrap_or(interface_string_index);
            alt.descriptor(
                CS_INTERFACE,
                &[
                    MIDI_OUT_JACK_SUBTYPE,
                    EMBEDDED,
                    out_jack_id_emb(i),
                    0x01,
                    in_jack_id_ext(i),
                    0x01,
                    jack_string_index.into(),
                ],
            );
        }

        let mut endpoint_data = [
            MS_GENERAL, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        endpoint_data[1] = n_out_jacks;
        for i in 0..n_out_jacks {
            endpoint_data[2 + i as usize] = in_jack_id_emb(i);
        }
        let read_ep = alt.endpoint_bulk_out(None, max_packet_size);
        alt.descriptor(CS_ENDPOINT, &endpoint_data[0..2 + n_out_jacks as usize]);

        endpoint_data[1] = n_in_jacks;
        for i in 0..n_in_jacks {
            endpoint_data[2 + i as usize] = out_jack_id_emb(i);
        }
        let write_ep = alt.endpoint_bulk_in(None, max_packet_size);
        alt.descriptor(CS_ENDPOINT, &endpoint_data[0..2 + n_in_jacks as usize]);

        Self { read_ep, write_ep }
    }

    async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }
}
