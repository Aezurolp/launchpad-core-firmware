#include <driver/driver.h>

/*
The Launchpad Mini Mk3 does not support velocity sensitivity or aftertouch.

These stubs are provided to allow compilation of code that expects these functions to exist.
*/

void driver_set_velocity_curve(uint8_t curve) { }
void driver_set_velocity_enabled(uint8_t enabled) { }

uint8_t driver_get_velocity_curve(void) { return 0; }
uint8_t driver_get_velocity_enabled(void) { return 0; }

void driver_set_aftertouch_curve(uint8_t curve) {}
void driver_set_aftertouch_mode(uint8_t mode) { }

uint8_t driver_get_aftertouch_curve(void) { }
uint8_t driver_get_aftertouch_mode(void) { }