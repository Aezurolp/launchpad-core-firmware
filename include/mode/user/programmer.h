#ifndef PROGRAMMER_H
#define PROGRAMMER_H

#include <stdint.h>

void programmer_init();
void programmer_timer_event();
void programmer_surface_event(uint8_t type, uint8_t index, uint8_t value);
void programmer_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
void programmer_aftertouch_event(uint8_t index, uint8_t value);

#endif