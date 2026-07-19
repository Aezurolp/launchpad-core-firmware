// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

#![no_std]
#![no_main]

pub mod app;
pub mod board;
pub mod leds;
pub mod grid;
pub mod runtime;
pub mod storage;
pub mod sysex;
pub mod usb;

use app::boot;
use esp_backtrace as _;
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_hal::efuse::{read_field_le, BLOCK_USR_DATA};
use esp_hal::otg_fs::Usb;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use firmware_core::app::{AftertouchEvent, AppHost, AppId, SurfaceEvent};
use firmware_core::{driver, sys::settings};
use static_cell::StaticCell;

use app::boot::BootApp;
use crate::app::live::LiveApp;
use crate::runtime::{Hardware, RuntimeDriver};
use crate::grid::HardwareEvent;

const LED_REFRESH_HZ: u16 = 300;

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    static HARDWARE: StaticCell<Hardware> = StaticCell::new();
    static RUNTIME_DRIVER: StaticCell<RuntimeDriver> = StaticCell::new();

    let peripherals = esp_hal::init(esp_hal::Config::default());
    let timer_group = TimerGroup::new(peripherals.TIMG0);
    let software_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timer_group.timer0, software_interrupt.software_interrupt0);

    crate::usb::init_event_queues();
    let usb = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);
    crate::usb::spawn(&spawner, crate::usb::make_driver(usb));

    let efuse_data: [u8; 10] = read_field_le(BLOCK_USR_DATA);
    let board = board::BoardConfig::from_efuse_user_data(&efuse_data);
    let grid = crate::grid::Grid::new(
        peripherals.ADC1,
        peripherals.GPIO1,
        peripherals.GPIO2,
        peripherals.GPIO3,
        peripherals.GPIO4,
        peripherals.GPIO5,
        peripherals.GPIO6,
        peripherals.GPIO7,
        peripherals.GPIO8,
        peripherals.GPIO9,
        peripherals.GPIO10,
        peripherals.GPIO12,
        peripherals.GPIO13,
        peripherals.GPIO14,
        peripherals.GPIO15,
        peripherals.GPIO16,
        peripherals.GPIO17,
        peripherals.GPIO21,
        peripherals.GPIO33,
        peripherals.GPIO34,
        peripherals.GPIO47,
        board,
    );
    let diagnostics = grid.fn_held();
    let hardware = HARDWARE.init(Hardware {
        leds: crate::leds::Leds::new(peripherals.RMT, peripherals.GPIO38),
        grid,
        flash: crate::storage::SettingsFlash::new(peripherals.FLASH),
    });
    let runtime = RUNTIME_DRIVER.init(RuntimeDriver::new(hardware as *mut Hardware));
    driver::install(runtime);
    settings::load();

    let mut host = AppHost::<BootApp, LiveApp, crate::sysex::Handler>::new(
        AppId::Boot,
        boot::new(),
        app::live::new(),
    );
    host.init();
    if diagnostics {
        hardware.leds.fill(0);
        for index in 0..96 {
            let logical = crate::grid::PHYSICAL_TO_LOGICAL[index];
            let color = if index < 64 { 0x002000 } else { 0x200020 };
            hardware.leds.set_rgb8(logical, color);
        }
    }

    let mut ticker = Ticker::every(Duration::from_millis(1));
    let mut now_ms = 0u32;
    let mut led_phase = 0u16;
    loop {
        ticker.next().await;
        now_ms = now_ms.wrapping_add(1);
        host.route_tick_event();

        if now_ms % 4 == 0 {
            hardware.grid.scan_controls(now_ms, now_ms % 8 == 0);
        }
        if diagnostics && now_ms % 16 == 0 {
            render_pressure_diagnostic(&mut hardware.leds, hardware.grid.pressure_levels());
        }
        while let Some(event) = hardware.grid.poll_event() {
            if diagnostics {
                render_diagnostic(&mut hardware.leds, event);
            } else {
                match event {
                    HardwareEvent::Surface {
                        pressed,
                        index,
                        value,
                    } => {
                        host.route_surface_event(SurfaceEvent {
                            pressed,
                            index,
                            value,
                        });
                    }
                    HardwareEvent::Aftertouch { index, value } => {
                        host.route_aftertouch_event(AftertouchEvent { index, value });
                    }
                }
            }
        }
        while let Some(event) = crate::usb::dequeue_midi_event() {
            if !diagnostics {
                host.route_midi_event(event);
            }
        }
        while let Some(message) = crate::usb::dequeue_sysex_message() {
            if !diagnostics {
                host.receive_sysex(message.port, &message.data[..message.len]);
            }
        }
        led_phase += LED_REFRESH_HZ;
        if led_phase >= 1_000 {
            led_phase -= 1_000;
            hardware.leds.show();
        }
    }
}

fn render_pressure_diagnostic(leds: &mut crate::leds::Leds, levels: &[u16; 64]) {
    for x in 0..8 {
        for y in 0..8 {
            let sensor = y * 8 + x;
            let intensity = ((levels[sensor] as u32 * 63)
                / crate::grid::HIGH_THRESHOLD as u32)
                .min(63) as u8;
            if let Some(index) = crate::grid::pad_logical_index(x, y) {
                leds.set_rgb6(index, intensity / 4, intensity, 0);
            }
        }
    }
}

fn render_diagnostic(leds: &mut crate::leds::Leds, event: HardwareEvent) {
    match event {
        HardwareEvent::Surface {
            pressed, index: 0, ..
        } => {
            leds.fill(if pressed { 0x202020 } else { 0 });
        }
        HardwareEvent::Surface {
            pressed,
            index,
            value,
        } => {
            leds.set_rgb8(index, if pressed { (value as u32) << 8 } else { 0 });
        }
        HardwareEvent::Aftertouch { index, value } => {
            leds.set_rgb8(index, (value as u32) << 8);
        }
    }
}
