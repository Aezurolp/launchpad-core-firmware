#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "lppmk3_leds.h"
#include "lppmk3_threading.h"
#include "app.h"

#define THUMB(addr) ((addr) | 1u)
#define FN(addr, type) ((type)(uintptr_t)THUMB(addr))

#define OFW_MIDI_RX_ADDR          0x08029924u
#define OFW_SYSEX_RELEASE_ADDR    0x08029A4Au
#define SEND_MIDI_ADDR            0x08024188u
#define SEND_MIDI_DIRECT_ADDR     0x08024110u

typedef int32_t (*ofw_midi_rx_fn)(int32_t portMask, char* msg, int32_t len);
typedef void    (*ofw_sysex_release_fn)(int32_t idx);
typedef int32_t (*send_midi_fn)(uint32_t portMask, const uint8_t* msg, uint32_t len);
typedef uint32_t (*send_midi_direct_fn)(uint32_t portMask, uint32_t len, const uint8_t* msg);

#define OFW_MIDI_RX       (FN(OFW_MIDI_RX_ADDR, ofw_midi_rx_fn))
#define OFW_SYSEX_RELEASE (FN(OFW_SYSEX_RELEASE_ADDR, ofw_sysex_release_fn))
#define SEND_MIDI         (FN(SEND_MIDI_ADDR, send_midi_fn))
#define SEND_MIDI_DIRECT  (FN(SEND_MIDI_DIRECT_ADDR, send_midi_direct_fn))

#define CONTROL_MAILBOX   (*(void**)0x2004fa6cu)

typedef int32_t (*osMailGet_fn)(int32_t* evt_out, void* mail_id, uint32_t timeout);
typedef int32_t (*osMailFree_fn)(void* mail_id, void* mail);
typedef int32_t (*osMailPut_fn)(void* mail_id, void* mail, uint32_t timeout);

#define OS_MAIL_GET   ((osMailGet_fn)(0x08015980u | 1u))
#define OS_MAIL_FREE  ((osMailFree_fn)(0x08015a18u | 1u))
#define OS_MAIL_PUT   ((osMailPut_fn)(0x08015916u | 1u))

static inline uint16_t rd_u16_le(const uint8_t* p) {
    return (uint16_t)p[0] | ((uint16_t)p[1] << 8);
}
static inline uint32_t rd_u32_le(const uint8_t* p) {
    return (uint32_t)p[0]
        | ((uint32_t)p[1] << 8)
        | ((uint32_t)p[2] << 16)
        | ((uint32_t)p[3] << 24);
}

static const uint8_t P3_XY[109] = {
    91, 92, 93, 94, 95, 96, 97, 98, 80, 70, 60, 50, 40, 30, 20, 10,
    89, 79, 69, 59, 49, 39, 29, 19, 1, 2, 3, 4, 5, 6, 7, 8,
    101, 102, 103, 104, 105, 106, 107, 108, 90, 0, 99,
    81, 82, 83, 84, 85, 86, 87, 88,
    71, 72, 73, 74, 75, 76, 77, 78,
    61, 62, 63, 64, 65, 66, 67, 68,
    51, 52, 53, 54, 55, 56, 57, 58,
    41, 42, 43, 44, 45, 46, 47, 48,
    31, 32, 33, 34, 35, 36, 37, 38,
    21, 22, 23, 24, 25, 26, 27, 28,
    11, 12, 13, 14, 15, 16, 17, 18
};

static inline uint8_t app_port_from_mask(uint8_t portMask)
{
    if (portMask == 0x10) return 2;

    return 1;
}

static inline bool is_control_msg(uint8_t t) {
    return (t == 0x03) || (t == 0x0C) || (t == 0x0D);
}
static inline bool is_midi_sysex(uint8_t t) { return (t == 0x07); }

void pump_controls_events(void)
{
    void* mbox = CONTROL_MAILBOX;
    int32_t evt[3];

    for (int i = 0; i < 32; i++) {
        OS_MAIL_GET(evt, mbox, 0);
        if (evt[0] != 0x20) break;

        uint8_t* msg = (uint8_t*)evt[1];
        if (!msg) break;

        const uint8_t t = msg[0];

        if (is_midi_sysex(t)) {
            const uint8_t portMask = msg[4];
            const uint8_t bufIdx   = msg[5];
            const uint16_t len     = rd_u16_le(&msg[6]);
            uint8_t* data          = (uint8_t*)(uintptr_t)rd_u32_le(&msg[8]);

            app_sysex_event(app_port_from_mask(portMask), data, len);

            OFW_SYSEX_RELEASE((int32_t)bufIdx);

            OS_MAIL_FREE(mbox, msg);
            continue;
        }

        // Controls
        if (is_control_msg(t)) {
            switch (t) {
                case 0x03: {
                    uint8_t idx = msg[4];
                    uint8_t pressed = msg[5];
                    if ((uint32_t)idx < 109u) {
                        app_surface_event(pressed ? 1u : 0u, P3_XY[idx], pressed ? 127u : 0u);
                    }
                    break;
                }
                case 0x0C: {
                    uint8_t pad = msg[4];
                    uint8_t pressed = msg[5];
                    uint16_t val = rd_u16_le(&msg[6]);
                    uint8_t xyIndex = (uint8_t)(pad + 43);
                    if ((uint32_t)xyIndex < 109u) {
                        app_surface_event(pressed ? 1u : 0u, P3_XY[xyIndex], pressed ? val : 0u);
                    }
                    break;
                }
                case 0x0D: {
                    uint8_t pad = msg[4];
                    uint16_t val = rd_u16_le(&msg[6]);
                    uint8_t xyIndex = (uint8_t)(pad + 43);
                    if ((uint32_t)xyIndex < 109u) {
                        app_aftertouch_event(P3_XY[xyIndex], val);
                    }
                    break;
                }
                default:
                    break;
            }

            OS_MAIL_FREE(mbox, msg);
            continue;
        }

        // Unknown msg type
        if (OS_MAIL_PUT(mbox, msg, 0) != 0) {
            OS_MAIL_FREE(mbox, msg);
        }
        return;
    }
}

extern uint32_t _sidata;
extern uint32_t _sdata;
extern uint32_t _edata;
extern uint32_t _sbss;
extern uint32_t _ebss;

#define CFW_STATE __attribute__((section(".cfw_state")))

CFW_STATE static uint32_t g_rt_magic0;
CFW_STATE static uint32_t g_rt_magic1;

static inline void cfw_runtime_init_once(void)
{
    const uint32_t M0 = 0xC0DEF00Du;
    const uint32_t M1 = 0x000385FFu;

    if (g_rt_magic0 == M0 && g_rt_magic1 == M1) return;

    uint32_t* src = &_sidata;
    uint32_t* dst = &_sdata;
    while (dst < &_edata) *dst++ = *src++;

    uint32_t* b = &_sbss;
    while (b < &_ebss) *b++ = 0u;

    g_rt_magic0 = M0;
    g_rt_magic1 = M1;
}

static uint32_t ms = 0;

__attribute__((section(".cfw_keep"), used))
void CFW_AppTick(void)
{
    cfw_runtime_init_once();

    static uint8_t init = 0;
    static uint16_t flush_div = 0;

    if (!init) {
        init = 1;
        lppmk3_leds_init();
        app_init();
    }

    pump_controls_events();

    if (flush_div >= 240u) {
        flush_div = 0;
        lppmk3_leds_flush();
    }

    if (flush_div++ % 60 == 0) {
        ms++;
        app_timer_event();
    }

    if (ms == 10000) {
        boost_m0();
    }
}

__attribute__((section(".cfw_keep"), used, noinline, aligned(4)))
int32_t CFW_MIDI_RECEIVE(int32_t arg1, char* arg2, int32_t arg3)
{
    if (arg1 != 4) return 0; // port 4 == the "MIDI" port
    if (!arg2 || arg3 <= 0) return 0;

    if ((uint8_t)arg2[0] == 0xF0) {
        return ((int32_t (*)(int32_t, char*, int32_t))(0x08029924u | 1u))(arg1, arg2, arg3);
    }

    if (arg3 == 3) {
        app_midi_event(1u, (uint8_t)arg2[0], (uint8_t)arg2[1], (uint8_t)arg2[2]);
        return 0;
    }

    return 0;
}

__attribute__((section(".cfw_keep"), used))
void driver_set_led(uint8_t led, uint32_t color)
{
    lppmk3_leds_set_xy(led, color);
}

__attribute__((section(".cfw_keep"), used))
void driver_send_midi(uint8_t port, const uint8_t* data, uint16_t len)
{
    (void)port;
    SEND_MIDI(4, data, len);
}

uint32_t driver_millis(void)
{
    return ms;
}
