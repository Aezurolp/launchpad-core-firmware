// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister
// Copyright (C) 2026 ZephyrCodesStuff

#![no_std]
#![no_main]

pub mod app;
pub mod boot;
pub mod extflash;
pub mod grid;
pub mod inputs;
pub mod led_scan;
pub mod leds;
pub mod runtime;
pub mod sysex;
pub mod usb;

use embassy_executor::Spawner;
use embassy_stm32::rcc::*;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use firmware_core::app::{AftertouchEvent, AppHost, AppId, SurfaceEvent};
use firmware_core::driver;
use firmware_core::sys::settings;
use panic_halt as _;
use static_cell::StaticCell;

type SharedAppHost =
    Mutex<CriticalSectionRawMutex, AppHost<boot::BootApp, app::live::LiveApp, sysex::Handler>>;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    static GRID: StaticCell<grid::Grid> = StaticCell::new();
    static RUNTIME_DRIVER: StaticCell<runtime::RuntimeDriver> = StaticCell::new();
    static APP_HOST: StaticCell<SharedAppHost> = StaticCell::new();

    let mut config = embassy_stm32::Config::default();

    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(24_000_000),
        mode: HseMode::Oscillator,
    });

    config.rcc.pll_src = PllSource::HSE;

    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV12,
        mul: PllMul::MUL168,
        divp: Some(PllPDiv::DIV4),
        divq: Some(PllQDiv::DIV7),
        divr: None,
    });

    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV1;
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;

    let p = embassy_stm32::init(config);

    usb::init_event_queues();

    let usb_driver = usb::make_driver(p.USB_OTG_FS, p.PA12, p.PA11);
    usb::spawn(&spawner, usb_driver);

    let grid = GRID.init(grid::Grid::new());
    let flash = extflash::ExtFlash::new(p.SPI2, p.PB13, p.PB15, p.PB14, p.PB12);
    let runtime_driver = RUNTIME_DRIVER.init(runtime::RuntimeDriver::new(grid, flash));
    driver::install(runtime_driver);
    settings::load();

    let app_host = APP_HOST.init(Mutex::new(AppHost::new(
        AppId::Boot,
        boot::new(),
        app::live::new(),
    )));
    app_host.lock().await.init();

    led_scan::start(grid);

    let mut ticker = Ticker::every(Duration::from_millis(1));

    loop {
        ticker.next().await;

        let mut app_host_guard = app_host.lock().await;
        app_host_guard.route_tick_event();

        grid.process_inputs();
        while let Some(event) = grid.poll_event() {
            match event {
                inputs::GridEvent::Press { note, value } => {
                    app_host_guard.route_surface_event(SurfaceEvent {
                        pressed: true,
                        index: note,
                        value,
                    });
                }
                inputs::GridEvent::Release { note } => {
                    app_host_guard.route_surface_event(SurfaceEvent {
                        pressed: false,
                        index: note,
                        value: 0,
                    });
                }
                inputs::GridEvent::Aftertouch { note, value } => {
                    app_host_guard.route_aftertouch_event(AftertouchEvent { index: note, value });
                }
            }
        }

        while let Some(event) = usb::dequeue_midi_event() {
            app_host_guard.route_midi_event(event);
        }

        while let Some(message) = usb::dequeue_sysex_message() {
            app_host_guard.receive_sysex(message.port, &message.data[..message.len]);
        }
    }
}
