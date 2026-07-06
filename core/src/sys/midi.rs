#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum MidiPort {
    Daw = 0,
    Midi = 1,
    Din = 2,
}

impl MidiPort {
    pub fn from_usb_cable(cable: u8) -> Option<Self> {
        match cable {
            0 => Some(Self::Daw),
            1 => Some(Self::Midi),
            _ => None,
        }
    }

    pub fn from_cable(cable: u8) -> Self {
        match cable {
            0 => Self::Daw,
            1 => Self::Midi,
            _ => Self::Din,
        }
    }

    pub fn as_cable(self) -> u8 {
        self as u8
    }
}
