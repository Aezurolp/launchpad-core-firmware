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
