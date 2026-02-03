#include "driver/driver.h"

#include <stdint.h>
#include <stddef.h>

#define FW_BASE         0x08003400u

#define SET_LED_ADDR    0x08004D89u

typedef void (*set_led_fn)(uint32_t a, uint8_t b, uint8_t c, uint8_t d, uint8_t e);

#define THUMB(addr) ((uintptr_t)(addr) | 1u)

#define SET_LED   ((set_led_fn)(SET_LED_ADDR | 1u))

static volatile uint8_t  * const g_midi_enable     = (volatile uint8_t  *)0x20000DEE;
static volatile uint32_t * const g_sysex_buf_ptr   = (volatile uint32_t *)0x20001010;
static volatile uint16_t * const g_sysex_max_len   = (volatile uint16_t *)0x20001016;
static volatile uint8_t  * const g_midi_port_flag  = (volatile uint8_t  *)0x20001018;
static volatile uint32_t * const g_midi_short_cb   = (volatile uint32_t *)0x20001020;
static volatile uint32_t * const g_midi_sysex_cb   = (volatile uint32_t *)0x20001024;

static uint8_t * const kSysexBuf = (uint8_t*)0x20000BCC;
static const uint16_t  kSysexMax = 0x021C;

static int32_t CFW_OnMidiShort(uint32_t meta_r5, uint8_t status, uint8_t d1, uint8_t d2, uint8_t d3)
{
    (void)meta_r5; // often encodes message/CIN/etc. keep for later digging

    uint8_t type = status & 0xF0;
    uint8_t ch   = status & 0x0F;

    // midi message handling
}

__attribute__((noinline))
static int32_t CFW_MidiShortCb_Stub(int32_t a1, int32_t a2, int32_t a3, int32_t a4)
{
    register uint32_t meta_r5 asm("r5");

    uint8_t status = (uint8_t)a1;
    uint8_t d1     = (uint8_t)a2;
    uint8_t d2     = (uint8_t)a3;
    uint8_t d3     = (uint8_t)a4;

    return CFW_OnMidiShort(meta_r5, status, d1, d2, d3);
}

static void CFW_MidiSysexCb(uint32_t a1, void *buf, int32_t len)
{
    (void)a1;

    uint8_t *b = (uint8_t*)buf;
    int32_t n = len;

    int32_t start = -1, end = -1;
    for (int32_t i = 0; i < n; i++) {
        if (start < 0 && b[i] == 0xF0) start = i;
        if (b[i] == 0xF7) end = i;
    }
    if (start < 0 || end < 0 || end <= start) return;

    uint8_t *syx = &b[start];
    int32_t syx_len = end - start + 1;

    // handle sysex
}

__attribute__((section(".cfw_keep")))
int32_t CFW_AppTick(){
    SET_LED(10, 63, 63, 63, 0);
    
    return 0;
}

__attribute__((section(".cfw_keep")))
int32_t SET_MIDI_CALLBACKS() {
    *g_midi_enable    = 1;
    *g_midi_port_flag = 1;

    *g_sysex_buf_ptr  = (uint32_t)kSysexBuf;
    *g_sysex_max_len  = kSysexMax;

    *g_midi_short_cb  = (uint32_t)THUMB(&CFW_MidiShortCb_Stub);
    *g_midi_sysex_cb  = (uint32_t)THUMB(&CFW_MidiSysexCb);

    return 0;
}
