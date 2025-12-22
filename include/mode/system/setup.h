#ifndef SETUP_H
#define SETUP_H

#include <stdint.h>

void setup_init();
void setup_timer_event();
void setup_surface_event(uint8_t type, uint8_t index, uint8_t value);
void setup_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
void setup_aftertouch_event(uint8_t index, uint8_t value);

#endif