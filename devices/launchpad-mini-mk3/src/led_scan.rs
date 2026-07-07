// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use embassy_stm32::interrupt::{self, InterruptExt};

use crate::grid::Grid;

const RCC_APB2ENR: *mut u32 = 0x4002_3844 as *mut u32;
const TIM1_CR1: *mut u32 = 0x4001_0000 as *mut u32;
const TIM1_DIER: *mut u32 = 0x4001_000c as *mut u32;
const TIM1_SR: *mut u32 = 0x4001_0010 as *mut u32;
const TIM1_EGR: *mut u32 = 0x4001_0014 as *mut u32;
const TIM1_CNT: *mut u32 = 0x4001_0024 as *mut u32;
const TIM1_PSC: *mut u32 = 0x4001_0028 as *mut u32;
const TIM1_ARR: *mut u32 = 0x4001_002c as *mut u32;

const RCC_APB2ENR_TIM1EN: u32 = 1 << 0;
const TIM_CR1_CEN: u32 = 1 << 0;
const TIM_DIER_UIE: u32 = 1 << 0;
const TIM_SR_UIF: u32 = 1 << 0;
const TIM_EGR_UG: u32 = 1 << 0;

const TIM1_PSC_1MHZ: u16 = 84 - 1;
const INITIAL_ARR_US: u16 = 32;
const MIN_IRQ_INTERVAL_US: u64 = 24;

static GRID: AtomicPtr<Grid<'static>> = AtomicPtr::new(ptr::null_mut());
static DRIVE_PHASE: AtomicBool = AtomicBool::new(false);
static FRAME_COMPLETE: AtomicBool = AtomicBool::new(false);

pub fn start(grid: *mut Grid<'static>) {
    GRID.store(grid, Ordering::Release);
    DRIVE_PHASE.store(false, Ordering::Relaxed);
    FRAME_COMPLETE.store(false, Ordering::Relaxed);

    unsafe {
        modify_reg(RCC_APB2ENR, |value| value | RCC_APB2ENR_TIM1EN);

        modify_reg(TIM1_CR1, |value| value & !TIM_CR1_CEN);
        write_reg(TIM1_PSC, TIM1_PSC_1MHZ as u32);
        write_reg(TIM1_ARR, INITIAL_ARR_US as u32);
        write_reg(TIM1_CNT, 0);
        write_reg(TIM1_EGR, TIM_EGR_UG);
        write_reg(TIM1_SR, 0);
        modify_reg(TIM1_DIER, |value| value | TIM_DIER_UIE);
    }

    interrupt::TIM1_UP_TIM10.unpend();
    unsafe {
        interrupt::TIM1_UP_TIM10.enable();
    }

    unsafe {
        modify_reg(TIM1_CR1, |value| value | TIM_CR1_CEN);
    }
}

pub fn take_frame_complete() -> bool {
    FRAME_COMPLETE.swap(false, Ordering::AcqRel)
}

#[cortex_m_rt::interrupt]
fn TIM1_UP_TIM10() {
    if unsafe { read_reg(TIM1_SR) } & TIM_SR_UIF == 0 {
        return;
    }
    unsafe {
        write_reg(TIM1_SR, 0);
    }

    let grid = GRID.load(Ordering::Acquire);
    if grid.is_null() {
        return;
    }

    let grid = unsafe { &mut *grid };

    if !DRIVE_PHASE.load(Ordering::Relaxed) {
        unsafe {
            write_reg(TIM1_ARR, timer_arr_from_us(grid.prepare_delay_us()) as u32);
        }
        grid.prepare_phase();
        DRIVE_PHASE.store(true, Ordering::Relaxed);
        return;
    }

    unsafe {
        write_reg(TIM1_ARR, timer_arr_from_us(grid.drive_delay_us()) as u32);
    }
    grid.drive_phase();
    grid.advance_slot();

    if grid.frame_complete() {
        FRAME_COMPLETE.store(true, Ordering::Release);
    }

    DRIVE_PHASE.store(false, Ordering::Relaxed);
}

fn timer_arr_from_us(us: u64) -> u16 {
    us.clamp(MIN_IRQ_INTERVAL_US, u16::MAX as u64) as u16
}

unsafe fn read_reg(reg: *mut u32) -> u32 {
    unsafe { ptr::read_volatile(reg) }
}

unsafe fn write_reg(reg: *mut u32, value: u32) {
    unsafe {
        ptr::write_volatile(reg, value);
    }
}

unsafe fn modify_reg(reg: *mut u32, f: impl FnOnce(u32) -> u32) {
    unsafe {
        let value = ptr::read_volatile(reg);
        ptr::write_volatile(reg, f(value));
    }
}
