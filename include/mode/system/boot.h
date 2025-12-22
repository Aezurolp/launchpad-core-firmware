#ifndef BOOT_H
#define BOOT_H

#include <stdint.h>

void boot_init();
void boot_timer_event();
void boot_surface_event(uint8_t type, uint8_t index, uint8_t value);
void boot_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
void boot_aftertouch_event(uint8_t index, uint8_t value);

#endif