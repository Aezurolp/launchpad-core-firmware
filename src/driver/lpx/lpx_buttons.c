#include <stdint.h>

#define LPX_VEL_ENABLED_ADDR   ((volatile uint8_t*)0x200005A8u)
#define LPX_VEL_CURVE_ADDR     ((volatile uint8_t*)0x200005AAu)
#define LPX_AT_MODE_ADDR       ((volatile uint8_t*)0x200005ACu)
#define LPX_AT_THRESH_RAW_ADDR ((volatile uint8_t*)0x200005ABu)

static inline uint8_t clamp3(uint8_t v){ return (v>2)?2:v; }

void driver_set_velocity_curve(uint8_t curve) {
    *LPX_VEL_CURVE_ADDR = clamp3(curve);
}

uint8_t driver_get_velocity_curve(void) {
    uint8_t v = *LPX_VEL_CURVE_ADDR;
    return (v<=2)?v:2;
}

void driver_set_velocity_enabled(uint8_t enabled) {
    *LPX_VEL_ENABLED_ADDR = enabled ? 1u : 0u;
}

uint8_t driver_get_velocity_enabled(void) {
    return (*LPX_VEL_ENABLED_ADDR) ? 1u : 0u;
}

void driver_set_aftertouch_mode(uint8_t mode) {
    *LPX_AT_MODE_ADDR = clamp3(mode);
}

uint8_t driver_get_aftertouch_mode(void) {
    uint8_t m = *LPX_AT_MODE_ADDR;
    return (m<=2)?m:0;
}

static inline uint8_t at_raw_from_curve(uint8_t curve){
    switch (curve) {
        default:
        case 0: return 0x20;
        case 1: return 0x40;
        case 2: return 0x60;
    }
}
static inline uint8_t at_curve_from_raw(uint8_t raw){
    if (raw < 0x30) return 0;
    if (raw < 0x50) return 1;
    return 2;
}

void driver_set_aftertouch_curve(uint8_t curve) {
    uint8_t raw = at_raw_from_curve(clamp3(curve)) & 0x7Fu;
    *LPX_AT_THRESH_RAW_ADDR = raw;
}
uint8_t driver_get_aftertouch_curve(void) {
    uint8_t raw = (*LPX_AT_THRESH_RAW_ADDR) & 0x7Fu;
    return at_curve_from_raw(raw);
}
