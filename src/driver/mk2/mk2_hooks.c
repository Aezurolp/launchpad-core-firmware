#include "driver/driver.h"

#include <stdint.h>
#include <stddef.h>

#define FW_BASE        0x08006400u

__attribute__((section(".cfw_keep")))
int32_t CFW_AppTick(){
    ((void (*)(uint32_t, uint8_t, uint8_t, uint8_t, uint8_t))0x08004D89)(10, 63, 63, 63, 0);
    return 0;
}

__attribute__((section(".cfw_keep")))
int32_t CFW_MIDI_RECEIVE(int32_t arg1, int32_t arg2, int32_t arg3, int32_t arg4) {
    ((void (*)(uint32_t, uint8_t, uint8_t, uint8_t, uint8_t))0x08004D89)(10, 63, 63, 63, 0);
    return 0;
}
