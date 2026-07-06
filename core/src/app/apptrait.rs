use crate::app::AppId;
use crate::app::events::{AftertouchEvent, MidiEvent, SurfaceEvent};

pub trait App {
    fn on_enter(&mut self);
    fn on_exit(&mut self);

    fn on_surface(&mut self, event: SurfaceEvent);
    fn on_midi(&mut self, event: MidiEvent);
    fn on_aftertouch(&mut self, event: AftertouchEvent);

    fn on_tick(&mut self);

    fn take_requested_app_switch(&mut self) -> Option<AppId> {
        None
    }
}
