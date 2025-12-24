#include "driver/driver.h"
#include "lpp_app.h"

void driver_set_velocity_curve(uint8_t curve) {

}

void driver_set_velocity_enabled(uint8_t enabled) {
    
}

uint8_t driver_get_velocity_curve(void) {
    return 0;
}

uint8_t driver_get_velocity_enabled(void) {
    return 0;
}

void driver_set_aftertouch_curve(uint8_t curve) {
    
}

void driver_set_aftertouch_mode(uint8_t mode) {
    
}

uint8_t driver_get_aftertouch_curve(void) {
    return 0;
}

uint8_t driver_get_aftertouch_mode(void) {
    return 0;
}

void driver_send_midi(uint8_t port, const uint8_t* data, uint16_t len) {
    if (len > 3) {
        hal_send_sysex(port, data, len);
    } else {
        hal_send_midi(port, data[0], data[1], data[2]);
    }
}