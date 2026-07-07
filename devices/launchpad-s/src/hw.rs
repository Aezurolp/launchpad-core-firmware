// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

pub const APP_VECTOR_TABLE: u32 = 0x0800_3000;
pub const SYSCLK_HZ: u32 = 48_000_000;

const RCC_BASE: usize = 0x4002_1000;
const FLASH_BASE: usize = 0x4002_2000;

const RCC_CR: *mut u32 = (RCC_BASE + 0x00) as *mut u32;
const RCC_CFGR: *mut u32 = (RCC_BASE + 0x04) as *mut u32;
const RCC_APB2ENR: *mut u32 = (RCC_BASE + 0x18) as *mut u32;
const RCC_APB1ENR: *mut u32 = (RCC_BASE + 0x1c) as *mut u32;
const FLASH_ACR: *mut u32 = FLASH_BASE as *mut u32;

const RCC_CR_HSEON: u32 = 1 << 16;
const RCC_CR_HSERDY: u32 = 1 << 17;
const RCC_CR_PLLON: u32 = 1 << 24;
const RCC_CR_PLLRDY: u32 = 1 << 25;

const RCC_CFGR_PLLSRC: u32 = 1 << 16;
const RCC_CFGR_PLLMUL8: u32 = 0b0110 << 18;
const RCC_CFGR_SW_PLL: u32 = 0b10;
const RCC_CFGR_SWS_PLL: u32 = 0b10 << 2;
const RCC_CFGR_PPRE1_DIV2: u32 = 0b100 << 8;
const RCC_CFGR_PPRE2_DIV2: u32 = 0b100 << 11;
const RCC_CFGR_USBPRE_PLL: u32 = 1 << 22;

const FLASH_ACR_LATENCY_1: u32 = 1;
const FLASH_ACR_PRFTBE: u32 = 1 << 4;

pub const RCC_APB2ENR_AFIOEN: u32 = 1 << 0;
pub const RCC_APB2ENR_IOPAEN: u32 = 1 << 2;
pub const RCC_APB2ENR_IOPBEN: u32 = 1 << 3;
pub const RCC_APB2ENR_IOPCEN: u32 = 1 << 4;
pub const RCC_APB2ENR_IOPDEN: u32 = 1 << 5;
pub const RCC_APB2ENR_SPI1EN: u32 = 1 << 12;
pub const RCC_APB1ENR_TIM2EN: u32 = 1 << 0;
pub const RCC_APB1ENR_TIM4EN: u32 = 1 << 2;
pub const RCC_APB1ENR_SPI2EN: u32 = 1 << 14;

pub const GPIOA_CRL: *mut u32 = 0x4001_0800 as *mut u32;
pub const GPIOA_CRH: *mut u32 = 0x4001_0804 as *mut u32;
pub const GPIOA_ODR: *mut u32 = 0x4001_080c as *mut u32;
pub const GPIOA_BSRR: *mut u32 = 0x4001_0810 as *mut u32;
pub const GPIOB_CRL: *mut u32 = 0x4001_0c00 as *mut u32;
pub const GPIOB_CRH: *mut u32 = 0x4001_0c04 as *mut u32;
pub const GPIOB_ODR: *mut u32 = 0x4001_0c0c as *mut u32;
pub const GPIOB_BSRR: *mut u32 = 0x4001_0c10 as *mut u32;
pub const GPIOC_CRL: *mut u32 = 0x4001_1000 as *mut u32;
pub const GPIOC_CRH: *mut u32 = 0x4001_1004 as *mut u32;
pub const GPIOD_CRL: *mut u32 = 0x4001_1400 as *mut u32;
pub const GPIOD_CRH: *mut u32 = 0x4001_1404 as *mut u32;

const TIM4_CR1: *mut u32 = 0x4000_0800 as *mut u32;
const TIM4_DIER: *mut u32 = 0x4000_080c as *mut u32;
const TIM4_SR: *mut u32 = 0x4000_0810 as *mut u32;
const TIM4_EGR: *mut u32 = 0x4000_0814 as *mut u32;
const TIM4_PSC: *mut u32 = 0x4000_0828 as *mut u32;
const TIM4_ARR: *mut u32 = 0x4000_082c as *mut u32;

const TIM_CR1_CEN: u32 = 1 << 0;
const TIM_DIER_UIE: u32 = 1 << 0;
const TIM_SR_UIF: u32 = 1 << 0;
const TIM_EGR_UG: u32 = 1 << 0;

pub const EVENT_1KHZ: u32 = 1 << 0;
pub const EVENT_200HZ: u32 = 1 << 1;
pub const EVENT_20HZ: u32 = 1 << 2;

static EVENTS: AtomicU32 = AtomicU32::new(0);
static TICK_DIV: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" {
    fn WWDG();
    fn PVD();
    fn TAMPER();
    fn RTC();
    fn FLASH();
    fn RCC();
    fn EXTI0();
    fn EXTI1();
    fn EXTI2();
    fn EXTI3();
    fn EXTI4();
    fn DMA1_CHANNEL1();
    fn DMA1_CHANNEL2();
    fn DMA1_CHANNEL3();
    fn DMA1_CHANNEL4();
    fn DMA1_CHANNEL5();
    fn DMA1_CHANNEL6();
    fn DMA1_CHANNEL7();
    fn ADC1_2();
    fn USB_HP_CAN_TX();
    fn USB_LP_CAN_RX0();
    fn CAN_RX1();
    fn CAN_SCE();
    fn EXTI9_5();
    fn TIM1_BRK();
    fn TIM1_UP();
    fn TIM1_TRG_COM();
    fn TIM1_CC();
    fn TIM2();
    fn TIM3();
    fn TIM4();
}

type Handler = unsafe extern "C" fn();

#[unsafe(link_section = ".vector_table.interrupts")]
#[unsafe(no_mangle)]
pub static __INTERRUPTS: [Handler; 31] = [
    WWDG,
    PVD,
    TAMPER,
    RTC,
    FLASH,
    RCC,
    EXTI0,
    EXTI1,
    EXTI2,
    EXTI3,
    EXTI4,
    DMA1_CHANNEL1,
    DMA1_CHANNEL2,
    DMA1_CHANNEL3,
    DMA1_CHANNEL4,
    DMA1_CHANNEL5,
    DMA1_CHANNEL6,
    DMA1_CHANNEL7,
    ADC1_2,
    USB_HP_CAN_TX,
    USB_LP_CAN_RX0,
    CAN_RX1,
    CAN_SCE,
    EXTI9_5,
    TIM1_BRK,
    TIM1_UP,
    TIM1_TRG_COM,
    TIM1_CC,
    TIM2,
    TIM3,
    TIM4,
];

#[derive(Copy, Clone)]
pub enum Interrupt {
    UsbLpCanRx0 = 20,
    Tim2 = 28,
    Tim4 = 30,
}

unsafe impl cortex_m::interrupt::InterruptNumber for Interrupt {
    fn number(self) -> u16 {
        self as u16
    }
}

pub fn init_clocks() {
    unsafe {
        write_reg(FLASH_ACR, FLASH_ACR_PRFTBE | FLASH_ACR_LATENCY_1);
        modify_reg(RCC_CR, |value| value | RCC_CR_HSEON);
        while read_reg(RCC_CR) & RCC_CR_HSERDY == 0 {}

        let cfgr = RCC_CFGR_PLLSRC
            | RCC_CFGR_PLLMUL8
            | RCC_CFGR_PPRE1_DIV2
            | RCC_CFGR_PPRE2_DIV2
            | RCC_CFGR_USBPRE_PLL;
        write_reg(RCC_CFGR, cfgr);
        modify_reg(RCC_CR, |value| value | RCC_CR_PLLON);
        while read_reg(RCC_CR) & RCC_CR_PLLRDY == 0 {}

        modify_reg(RCC_CFGR, |value| (value & !0b11) | RCC_CFGR_SW_PLL);
        while read_reg(RCC_CFGR) & (0b11 << 2) != RCC_CFGR_SWS_PLL {}
    }
}

pub fn init_gpio_clocks() {
    unsafe {
        modify_reg(RCC_APB2ENR, |value| {
            value
                | RCC_APB2ENR_AFIOEN
                | RCC_APB2ENR_IOPAEN
                | RCC_APB2ENR_IOPBEN
                | RCC_APB2ENR_IOPCEN
                | RCC_APB2ENR_IOPDEN
        });
    }
}

pub fn init_tick_timer() {
    unsafe {
        modify_reg(RCC_APB1ENR, |value| value | RCC_APB1ENR_TIM4EN);
        write_reg(TIM4_CR1, 0);
        write_reg(TIM4_PSC, 48 - 1);
        write_reg(TIM4_ARR, 1000 - 1);
        write_reg(TIM4_EGR, TIM_EGR_UG);
        write_reg(TIM4_SR, 0);
        write_reg(TIM4_DIER, TIM_DIER_UIE);
    }

    unsafe {
        core::ptr::write_volatile((0xE000_E400 + Interrupt::Tim4 as u32) as *mut u8, 0x80);
        cortex_m::peripheral::NVIC::unmask(Interrupt::Tim4);
    }

    unsafe {
        modify_reg(TIM4_CR1, |value| value | TIM_CR1_CEN);
    }
}

pub fn take_events(mask: u32) -> u32 {
    EVENTS.fetch_and(!mask, Ordering::AcqRel) & mask
}

#[unsafe(export_name = "TIM4")]
pub extern "C" fn tim4_handler() {
    if unsafe { read_reg(TIM4_SR) } & TIM_SR_UIF == 0 {
        return;
    }

    let tick = TICK_DIV.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    let mut events = EVENT_1KHZ;
    if tick % 5 == 0 {
        events |= EVENT_200HZ;
    }
    if tick % 50 == 0 {
        events |= EVENT_20HZ;
    }

    EVENTS.fetch_or(events, Ordering::Release);

    unsafe {
        write_reg(TIM4_SR, 0);
    }
}

pub unsafe fn read_reg(reg: *mut u32) -> u32 {
    unsafe { ptr::read_volatile(reg) }
}

pub unsafe fn write_reg(reg: *mut u32, value: u32) {
    unsafe {
        ptr::write_volatile(reg, value);
    }
}

pub unsafe fn modify_reg(reg: *mut u32, f: impl FnOnce(u32) -> u32) {
    let value = unsafe { ptr::read_volatile(reg) };
    unsafe {
        ptr::write_volatile(reg, f(value));
    }
}
