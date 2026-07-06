pub trait LedTarget {
    fn set_palette(&mut self, index: u8, velocity: u8);
    fn set_rgb(&mut self, index: u8, r: u8, g: u8, b: u8);
}

pub fn handle_modern(data: &[u8], device_id: u8, target: &mut impl LedTarget) -> bool {
    if data.len() < 8 || data[0] != 0xf0 || data.last() != Some(&0xf7) {
        return false;
    }
    if !matches!(
        data,
        [0xf0, 0x00, 0x20, 0x29, 0x02, id, 0x03, ..] if *id == device_id
    ) {
        return false;
    }

    let mut index = 7;
    while index < data.len() - 1 {
        if index + 2 > data.len() - 1 {
            break;
        }

        let lighting_type = data[index];
        let led_index = data[index + 1];
        index += 2;

        match lighting_type {
            0 => {
                if index >= data.len() - 1 {
                    break;
                }
                target.set_palette(led_index, data[index]);
                index += 1;
            }
            1 => {
                if index + 2 > data.len() - 1 {
                    break;
                }
                index += 2;
            }
            2 => {
                if index + 1 > data.len() - 1 {
                    break;
                }
                index += 1;
            }
            3 => {
                if index + 3 > data.len() - 1 {
                    break;
                }
                let r = data[index] & 0x3f;
                let g = data[index + 1] & 0x3f;
                let b = data[index + 2] & 0x3f;
                index += 3;
                target.set_rgb(led_index, r, g, b);
            }
            _ => break,
        }
    }

    true
}
