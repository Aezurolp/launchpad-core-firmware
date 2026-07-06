pub mod device_inquiry;

use firmware_core::app::AppId;
use firmware_core::sys::midi::MidiPort;
use firmware_core::sys::sysex::{DefaultSysExHandler, SysExHandler};

pub struct Handler;

impl SysExHandler for Handler {
    fn execute(app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if device_inquiry::Handler::execute(app, port, data) {
            return true;
        }

        DefaultSysExHandler::execute(app, port, data)
    }
}
