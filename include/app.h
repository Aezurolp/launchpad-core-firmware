// App framework inspired by the original open source launchpad pro firmware

#ifndef APP_H
#define APP_H

#include <stdint.h>

#if defined(LPPRO)
void alt_app_init();
void alt_app_timer_event();
void alt_app_surface_event(uint8_t type, uint8_t index, uint8_t value);
void alt_app_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
void alt_app_aftertouch_event(uint8_t index, uint8_t value);
void alt_app_sysex_event(uint8_t port, uint8_t* buf, uint16_t len);
#else
void app_init();
void app_timer_event();
void app_surface_event(uint8_t type, uint8_t index, uint8_t value);
void app_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
void app_aftertouch_event(uint8_t index, uint8_t value);
void app_sysex_event(uint8_t port, uint8_t* buf, uint16_t len);
#endif

#endif