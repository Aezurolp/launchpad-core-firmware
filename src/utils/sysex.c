#include "utils/sysex.h"
#include "driver/driver.h"
#include "driver/sysconf.h"
#include "flash/settings.h"
#include "flash/flash.h"
#include "utils/palette.h"
#include "mode/mode.h"
#include "led/led.h"

#if defined(LPX)
#define DEVICE_ID 0x03
#define DEVICE_INQUIRY_RESPONSE { 240, 126, 0, 6, 2, 0, 32, 41, 3, 1, 0, 0, 0, 9, 9, 9, 247 }
#elif defined(LPMINI)
#define DEVICE_ID 19
#define DEVICE_INQUIRY_RESPONSE { 240, 126, 0, 6, 2, 0, 32, 41, 19, 1, 0, 0, 0, 9, 9, 9, 247 }
#elif defined(LPPRO)
#define DEVICE_ID 81
#define DEVICE_INQUIRY_RESPONSE { 240, 126, 0, 6, 2, 0, 32, 41, 81, 0, 0, 0, 0, 99, 102, 121, 247 }
#elif defined(LPPMK3)
#define DEVICE_ID 35
#define DEVICE_INQUIRY_RESPONSE { 240, 126, 0, 6, 2, 0, 32, 41, 35, 1, 0, 0, 0, 9, 9, 9, 247 }
#else
#define DEVICE_ID 0x0
#define DEVICE_INQUIRY_RESPONSE { 240, 126, 0, 6, 2, 0, 32, 41, 3, 1, 0, 0, 0, 3, 5, 2, 247 }
#endif

#define DEVICE_INQUIRY_LENGTH 17

#if defined(LPPRO)
static inline int map_grid_rc_to_led(uint8_t row, uint8_t col) {
    if (row > 9 || col > 9) return -1;
    if ((row == 0 || row == 9) && (col == 0 || col == 9)) return -1;
    
    return (int)(row * 10 + col);
}
#endif

void handle_sysex(uint8_t* buf, uint16_t len) {
    if (*buf != 0xF0) return;

    // Device Inquiry
    if (len == 6 && buf[1] == 0x7E && buf[2] == 0x7F && buf[3] == 0x06 && buf[4] == 0x01) {;
        uint8_t response[DEVICE_INQUIRY_LENGTH] = DEVICE_INQUIRY_RESPONSE;

        driver_send_midi(1, response, DEVICE_INQUIRY_LENGTH);
    }

    // CFW Version Inquiry:
    // Request:  F0 00 20 29 02 7F 00 F7
    // Response: F0 00 20 29 02 7F 01 <device_type> <major> <minor> <patch> F7
    if (len >= 7 &&
        buf[1] == 0x00 && buf[2] == 0x20 && buf[3] == 0x29 && buf[4] == 0x02 &&
        buf[5] == 0x7F && buf[6] == 0x00) {
        const uint8_t ver[3] = CFW_VERSION; // {major, minor, patch}
        uint8_t resp[12] = {
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x7F, 0x01,
            DEVICE_ID,
            (uint8_t)(ver[0] & 0x7F), (uint8_t)(ver[1] & 0x7F), (uint8_t)(ver[2] & 0x7F),
            0xF7
        };

        driver_send_midi(1, resp, sizeof(resp));
        return;
    }

    if (buf[1] == 0x5F) {// FASTLED by mat1jaczyyy
        for (uint8_t* i = buf + 2; i < buf + (len - 1);) {
            uint8_t r = *i++;
            uint8_t g = *i++;
            uint8_t b = *i++;

            uint8_t n = ((r & 0x40) >> 4) | ((g & 0x40) >> 5) | ((b & 0x40) >> 6);
            if (n == 0) n = *i++;

            r &= 0x3F;
            g &= 0x3F;
            b &= 0x3F;

            #if defined(LPMINI) || defined(LPX) || defined(LPPMK3) 
            r *= 2;
            g *= 2;
            b *= 2;
            #endif

            for (uint8_t j = 0; j < n; j++) {
                uint8_t x = *i++;

                if (x == 0)
                    for (uint8_t k = 0; k < 99; k++)
                        rgb_led(k, r, g, b);

                else if (x <= 99)
                    #if defined(LPPMK3)
                        if (x >= 1 && x <= 8) {
                            rgb_led(100 + x, r, g, b);
                            rgb_led(x, r, g, b);
                        } else {
                            rgb_led(x, r, g, b);
                        }
                    #else
                        rgb_led(x, r, g, b);
                    #endif

                else if (x <= 109) {
                    x = (x - 100) * 10 + 1;

                    for (uint8_t k = x; k < x + 8; k++)
                        rgb_led(k, r, g, b);

                } else if (x <= 119) {
                    x -= 100;

                    for (uint8_t k = x; k < 90; k += 10)
                        rgb_led(k, r, g, b);
                }
            }
        }
    }

    if (buf[1] == 0x52) { // Palette flash
        uint8_t palette_index = buf[2];
        uint8_t write_mode = buf[3];
        uint8_t color_space = buf[4] % 3;

        if (write_mode) { // Write
            for (uint8_t i = 0; i < 128; i++) {
                if (palette_index <= 3) {
                    settings_custom_palette[palette_index][color_space][i] = buf[5 + i];
                } else {
                    temporary_palette[color_space][i] = buf[5 + i];
                }
            }

            flash_write();
        }
    }

    if (len == 9) {
        if (buf[1] == 0x00 && buf[2] == 0x20 && buf[3] == 0x29 && buf[4] == 0x02) {
            uint8_t mode = buf[7];

            if (mode == 0) {
                mode_switch(MODE_PERFORMANCE);
            }
        }
    }

    #if !defined(LPPRO) // LED SYSTEM FOR LPX, LPMINI, LPPMK3
    if (len >= 8 &&
        buf[1] == 0x00 && buf[2] == 0x20 && buf[3] == 0x29 && buf[4] == 0x02 &&
        (buf[5] == 0x0C || buf[5] == 0x0D || buf[5] == 0x0E) && buf[6] == 0x03) {
        
        if (current_mode != MODE_PERFORMANCE && current_mode != MODE_PROGRAMMER) return;
        
        uint16_t i = 7; // start of <Colour Spec> list
        while (i < (uint16_t)(len - 1)) { // stop before F7
            if (i + 2 > (uint16_t)(len - 1)) break; // need type + index
            uint8_t lighting_type = buf[i++];
            uint8_t led_index     = buf[i++];

            if (lighting_type == 0) {
                // Static palette: 1 byte
                if (i >= (uint16_t)(len - 1)) break;
                uint8_t palette_entry = buf[i++];
                palette_led(led_index, palette_entry);
            } else if (lighting_type == 1) {
                // Flashing colour (B, A): not supported yet
                if (i + 2 > (uint16_t)(len - 1)) break;
                i += 2;
            } else if (lighting_type == 2) {
                // Pulsing colour (palette entry): not supported yet
                if (i + 1 > (uint16_t)(len - 1)) break;
                i += 1;
            } else if (lighting_type == 3) {
                // RGB: 3 bytes (R,G,B) 0..127
                if (i + 3 > (uint16_t)(len - 1)) break;
                uint8_t r = buf[i++];
                uint8_t g = buf[i++];
                uint8_t b = buf[i++];
                rgb_led(led_index, r, g, b);
            } else {
                break;
            }
        }

        return;
    }
    #else // LED SYSTEM FOR LPPRO
    if (len >= 8 &&
        buf[1] == 0x00 && buf[2] == 0x20 && buf[3] == 0x29 && buf[4] == 0x02 && buf[5] == 0x10) {
        
        if (current_mode != MODE_PERFORMANCE && current_mode != MODE_PROGRAMMER) return;
        
        uint8_t cmd = buf[6];
        
        if (cmd == 0x0A) {
            // Light LED using SysEx (palette/indexed colour)
            // F0 00 20 29 02 10 0A <LED> <Colour> ... F7
            // <LED> <Colour> pairs may be repeated up to 97 times
            uint16_t i = 7;
            while (i + 1 < (uint16_t)(len - 1)) {
                uint8_t led = buf[i++];
                uint8_t colour = buf[i++];
                
                if (led <= 99) {
                    novation_led(led, colour);
                }
            }
            return;
        }
        
        if (cmd == 0x0B) {
            // Light LED using SysEx (RGB mode)
            // F0 00 20 29 02 10 0B <LED> <Red> <Green> <Blue> ... F7
            // RGB values 0..63 (0x00..0x3F), groups may be repeated up to 78 times
            uint16_t i = 7;
            while (i + 3 < (uint16_t)(len - 1)) {
                uint8_t led = buf[i++];
                uint8_t r = buf[i++] & 0x3F;
                uint8_t g = buf[i++] & 0x3F;
                uint8_t b = buf[i++] & 0x3F;
                
                if (led <= 99) {
                    rgb_led(led, r, g, b);
                }
            }
            return;
        }
        
        if (cmd == 0x0C) {
            if (len < 9) return;
            uint8_t col = buf[7];
            if (col > 9) return;
            
            uint16_t i = 8;
            uint8_t row = 0;
            while (i < (uint16_t)(len - 1) && row <= 9) {
                uint8_t colour = buf[i++];
                int led = map_grid_rc_to_led(row, col);
                if (led >= 0) {
                    novation_led((uint8_t)led, colour);
                }
                row++;
            }
            return;
        }
        
        if (cmd == 0x0D) {
            if (len < 9) return;
            uint8_t row = buf[7];
            if (row > 9) return;
            
            uint16_t i = 8;
            uint8_t col = 0;
            while (i < (uint16_t)(len - 1) && col <= 9) {
                uint8_t colour = buf[i++];
                int led = map_grid_rc_to_led(row, col);
                if (led >= 0) {
                    novation_led((uint8_t)led, colour);
                }
                col++;
            }
            return;
        }
        
        if (cmd == 0x0F) {
            if (len < 11) return;
            uint8_t grid_type = buf[7];
            uint16_t i = 8;
            
            if (grid_type == 0) {
                for (uint8_t row = 0; row <= 9; row++) {
                    for (uint8_t col = 0; col <= 9; col++) {
                        if (i + 2 >= (uint16_t)(len - 1)) return;
                        uint8_t r = buf[i++] & 0x3F;
                        uint8_t g = buf[i++] & 0x3F;
                        uint8_t b = buf[i++] & 0x3F;
                        
                        int led = map_grid_rc_to_led(row, col);
                        if (led >= 0) {
                            rgb_led((uint8_t)led, r, g, b);
                        }
                    }
                }
                return;
            }
            
            if (grid_type == 1) {
                for (uint8_t row = 0; row < 8; row++) {
                    for (uint8_t col = 0; col < 8; col++) {
                        if (i + 2 >= (uint16_t)(len - 1)) return;
                        uint8_t r = buf[i++] & 0x3F;
                        uint8_t g = buf[i++] & 0x3F;
                        uint8_t b = buf[i++] & 0x3F;
                        
                        int led = map_grid_rc_to_led((uint8_t)(row + 1), (uint8_t)(col + 1));
                        if (led >= 0) {
                            rgb_led((uint8_t)led, r, g, b);
                        }
                    }
                }
                return;
            }
            
            return;
        }
    }
    #endif
}