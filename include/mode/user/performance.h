#ifndef PERFORMANCE_H
#define PERFORMANCE_H

#include <stdint.h>

void performance_init();
void performance_timer_event();
void performance_surface_event(uint8_t type, uint8_t index, uint8_t value);
void performance_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
void performance_aftertouch_event(uint8_t index, uint8_t value);

#endif