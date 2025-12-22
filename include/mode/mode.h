#ifndef MODE_H
#define MODE_H

#include <stdint.h>

extern uint8_t current_mode;
extern uint8_t mode;

#define MODES_COUNT 6

#define MODE_BOOT 0
#include "mode/system/boot.h"

#define MODE_SETUP 1
#include "mode/system/setup.h"

#define MODE_PERFORMANCE 2
#include "mode/user/performance.h"

#define MODE_PROGRAMMER 3
#include "mode/user/programmer.h"

struct Mode {
    char * name;
    uint32_t color;
    uint32_t color_dimmed;
    void (*init)();
    void (*timer_event)();
    void (*surface_event)(uint8_t type, uint8_t index, uint8_t value);
    void (*midi_event)(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2);
    void (*aftertouch_event)(uint8_t index, uint8_t value);
};

extern const struct Mode modes[MODES_COUNT];

void mode_switch(uint8_t m);
void mode_refresh();

#endif
