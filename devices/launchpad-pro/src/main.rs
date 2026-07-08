// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

#![no_std]
#![no_main]

pub mod app;
pub mod boot;
pub mod flash;
pub mod grid;
pub mod inputs;
pub mod leds;
pub mod runtime;
pub mod sysex;
pub mod usb;

use embassy_executor::Spawner;
use embassy_stm32::interrupt::InterruptExt;
use embassy_stm32::rcc::*;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use firmware_core::app::{AftertouchEvent, AppHost, AppId, SurfaceEvent};
use firmware_core::driver;
use firmware_core::sys::settings;
use panic_halt as _;
use static_cell::StaticCell;

const APP_VECTOR_TABLE: u32 = 0x0800_6400;

type SharedAppHost =
    Mutex<CriticalSectionRawMutex, AppHost<boot::BootApp, app::live::LiveApp, sysex::Handler>>;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    static GRID: StaticCell<grid::Grid> = StaticCell::new();
    static RUNTIME_DRIVER: StaticCell<runtime::RuntimeDriver> = StaticCell::new();
    static APP_HOST: StaticCell<SharedAppHost> = StaticCell::new();

    unsafe {
        (*cortex_m::peripheral::SCB::PTR)
            .vtor
            .write(APP_VECTOR_TABLE);
    }

    let mut config = embassy_stm32::Config::default();
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::Hertz(6_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll = Some(Pll {
        src: PllSource::HSE,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL12,
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV2;
    config.rcc.apb2_pre = APBPrescaler::DIV1;

    let _p = embassy_stm32::init(config);

    // embassy leaves its TIM4 time-driver interrupt at the reset-default
    // preemption priority (P0). Our BASEPRI critical section (see
    // `leds::ScanPriorityCriticalSection`) reserves P0 exclusively for the LED
    // scan and only masks P1..P15, so every embassy-managed interrupt must sit
    // at >= P1 to stay covered by critical sections. Bump TIM4 to P1 before any
    // timer alarm can fire concurrently with a critical section.
    embassy_stm32::interrupt::TIM4.set_priority(embassy_stm32::interrupt::Priority::P1);

    init_usb_board();

    usb::init_event_queues();
    usb::init();

    let grid = GRID.init(grid::Grid::new());
    let flash = flash::Flash::new();
    let runtime_driver = RUNTIME_DRIVER.init(runtime::RuntimeDriver::new(grid, flash));
    driver::install(runtime_driver);
    settings::load();

    let app_host = APP_HOST.init(Mutex::new(AppHost::new(
        AppId::Boot,
        boot::new(),
        app::live::new(),
    )));
    app_host.lock().await.init();
    leds::start_scan(grid as *mut grid::Grid);
    grid.start_adc_scan();

    let mut ticker = Ticker::every(Duration::from_millis(1));
    let mut tick_200hz_divider = 0u8;

    loop {
        ticker.next().await;

        grid.tick_1khz_collect();

        tick_200hz_divider = (tick_200hz_divider + 1) % 5;
        if tick_200hz_divider == 0 {
            grid.tick_200hz();
        }

        let mut app_host_guard = app_host.lock().await;

        app_host_guard.route_tick_event();

        while let Some(event) = grid.poll_event() {
            match event {
                inputs::GridEvent::Press { index, value } => {
                    app_host_guard.route_surface_event(SurfaceEvent {
                        pressed: true,
                        index,
                        value,
                    });
                }
                inputs::GridEvent::Release { index } => {
                    app_host_guard.route_surface_event(SurfaceEvent {
                        pressed: false,
                        index,
                        value: 0,
                    });
                }
                inputs::GridEvent::Aftertouch { index, value } => {
                    app_host_guard.route_aftertouch_event(AftertouchEvent { index, value });
                }
            }
        }

        while let Some(event) = usb::dequeue_midi_event() {
            app_host_guard.route_midi_event(event);
        }

        while let Some(message) = usb::dequeue_sysex_message() {
            app_host_guard.receive_sysex(message.port, &message.data[..message.len]);
        }

        drop(app_host_guard);

        grid.tick_1khz_start();
    }
}

fn init_usb_board() {
    const RCC_APB2ENR: *mut u32 = 0x4002_1018 as *mut u32;
    const GPIOA_CRH: *mut u32 = 0x4001_0804 as *mut u32;
    const GPIOA_BSRR: *mut u32 = 0x4001_0810 as *mut u32;
    const GPIOA_BRR: *mut u32 = 0x4001_0814 as *mut u32;
    const GPIOD_CRL: *mut u32 = 0x4001_1400 as *mut u32;
    const GPIOD_BSRR: *mut u32 = 0x4001_1410 as *mut u32;
    const IOPAEN: u32 = 1 << 2;
    const IOPDEN: u32 = 1 << 5;
    const PA10_PA12_MODE_MASK: u32 = 0xfff << 8;
    const PA10_OUTPUT_PA11_PA12_FLOATING: u32 = (0x2 << 8) | (0x4 << 12) | (0x4 << 16);
    const PD4_MODE_MASK: u32 = 0xf << 16;
    const PD4_OUTPUT_2MHZ: u32 = 0x2 << 16;

    unsafe {
        core::ptr::write_volatile(
            RCC_APB2ENR,
            core::ptr::read_volatile(RCC_APB2ENR) | IOPAEN | IOPDEN,
        );
        core::ptr::write_volatile(
            GPIOA_CRH,
            (core::ptr::read_volatile(GPIOA_CRH) & !PA10_PA12_MODE_MASK)
                | PA10_OUTPUT_PA11_PA12_FLOATING,
        );
        core::ptr::write_volatile(GPIOA_BRR, 1 << 10);
        cortex_m::asm::delay(720_000);
        core::ptr::write_volatile(GPIOA_BSRR, 1 << 10);

        core::ptr::write_volatile(
            GPIOD_CRL,
            (core::ptr::read_volatile(GPIOD_CRL) & !PD4_MODE_MASK) | PD4_OUTPUT_2MHZ,
        );
        core::ptr::write_volatile(GPIOD_BSRR, 1 << 4);
    }
}
