// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use crate::app::SurfaceEvent;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum PageId {
    Init,
    Leds,
    #[cfg(feature = "pressure-sensitive")]
    Velocity,
    #[cfg(feature = "pressure-sensitive")]
    Aftertouch,
}

/// Pages for the setup app. They have a smaller version of the core App framework.
pub trait Page {
    fn on_enter(&mut self);

    fn on_surface(&mut self, event: SurfaceEvent);

    fn on_tick(&mut self) {}
}
