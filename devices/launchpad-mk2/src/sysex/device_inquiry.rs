use firmware_core::app::AppId;
use firmware_core::sys::midi::MidiPort;
use firmware_core::sys::sysex::SysExHandler;

pub struct Handler;

impl SysExHandler for Handler {
    fn execute(_app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if data == &[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7] {
            let response = [
                0xf0, 0x7e, 0x00, 0x06, 0x02, 0x00, 0x20, 0x29, 105, 0, 0, 0, 0, 9, 9, 9, 0xf7,
            ];

            firmware_core::driver::send_midi(port, &response);

            true
        } else {
            false
        }
    }
}
