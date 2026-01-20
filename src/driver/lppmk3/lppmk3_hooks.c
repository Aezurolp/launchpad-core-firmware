#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "lppmk3_leds.h"
#include "lppmk3_threading.h"
#include "app.h"

#define FW_BASE          0x08010000u
#define SEND_MIDI_ADDR   (0x08024188u)

#define UPDATE_KEYSCAN_ADDR   0x080292e2u
#define RAW_BUTTON_STATE_ADDR 0x0802516au

typedef void     (*update_keyscan_fn)(uint32_t* dst);
typedef int32_t  (*raw_button_state_fn)(int32_t idx);

#define UPDATE_KEYSCAN    ((update_keyscan_fn)(UPDATE_KEYSCAN_ADDR   | 1u))
#define RAW_BUTTON_STATE  ((raw_button_state_fn)(RAW_BUTTON_STATE_ADDR | 1u))

#define KEYSCAN_BUF ((uint32_t*)0x20002868u)

#define NUM_INPUTS 0x2A

#define ANALOG_SCAN_ADDR     0x0802546cu
#define ANALOG_EVT_BUF_ADDR  0x200022f8u

typedef uint32_t (*analog_scan_fn)(uint8_t *buffer);
#define ANALOG_SCAN    ((analog_scan_fn)(ANALOG_SCAN_ADDR | 1u))
#define ANALOG_EVT_BUF ((uint8_t*)ANALOG_EVT_BUF_ADDR)

#define NUM_PADS 64

#define SEND_MIDI_DIRECT_ADDR  0x08024110u

typedef int32_t  (*send_midi_fn)(uint32_t portMask, const uint8_t* msg, uint32_t len);
typedef uint32_t (*send_midi_direct_fn)(uint32_t portMask, uint32_t len, const uint8_t* msg);

#define SEND_MIDI        ((send_midi_fn)(SEND_MIDI_ADDR | 1u))
#define SEND_MIDI_DIRECT ((send_midi_direct_fn)(SEND_MIDI_DIRECT_ADDR | 1u))

#define SET_BTN_CB_ADDR      0x08060534u
#define SET_PAD_CB_ADDR      0x08060408u
#define SET_PADPRES_CB_ADDR  0x0806048Cu

typedef void (*btn_cb_t)(int32_t idx, int32_t pressed, void* ctx);
typedef void (*pad_cb_t)(int32_t velocity, int32_t pad, int32_t pressed, void* ctx);
typedef void (*padpres_cb_t)(int32_t pad, int32_t pressure, void* ctx);

typedef int32_t (*set_btn_cb_fn)(btn_cb_t cb, void* ctx);
typedef int32_t (*set_pad_cb_fn)(pad_cb_t cb, void* ctx);
typedef int32_t (*set_padpres_cb_fn)(padpres_cb_t cb, void* ctx);

#define SET_BTN_CB     ((set_btn_cb_fn)(SET_BTN_CB_ADDR | 1u))
#define SET_PAD_CB     ((set_pad_cb_fn)(SET_PAD_CB_ADDR | 1u))
#define SET_PADPRES_CB ((set_padpres_cb_fn)(SET_PADPRES_CB_ADDR | 1u))

#define SYSEX_SEM_PTR_ADDR 0x2004ea98u
#define SYSEX_SEM          (*(void**)SYSEX_SEM_PTR_ADDR)

typedef int32_t (*os_sem_give_fn)(void* sem);
#define OS_SEM_GIVE ((os_sem_give_fn)(0x080155BAu | 1u))

typedef int32_t (*ofw_set_prio_fn)(uint32_t thread_id, uint32_t prio);
#define OFW_SET_PRIO FN(0x0801534e, ofw_set_prio_fn)

static inline uint32_t M0TX_THREAD_ID(void) { return *(volatile uint32_t*)0x2004FA28u; }

static const uint8_t P3_XY[109] = { 91, 92, 93, 94, 95, 96, 97, 98, 80, 70, 60, 50, 40, 30, 20, 10, 89, 79, 69, 59, 49, 39, 29, 19, 1, 2, 3, 4, 5, 6, 7, 8, 101, 102, 103, 104, 105, 106, 107, 108, 90, 0, 99, 81, 82, 83, 84, 85, 86, 87, 88, 71, 72, 73, 74, 75, 76, 77, 78, 61, 62, 63, 64, 65, 66, 67, 68, 51, 52, 53, 54, 55, 56, 57, 58, 41, 42, 43, 44, 45, 46, 47, 48, 31, 32, 33, 34, 35, 36, 37, 38, 21, 22, 23, 24, 25, 26, 27, 28, 11, 12, 13, 14, 15, 16, 17, 18 };

__attribute__((section(".cfw_state"), aligned(4)))
static uint8_t input_prev[128];

static void on_button(int32_t idx, int32_t pressed, void* ctx)
{
    (void)ctx;
    if ((uint32_t)idx < 43u) {
        app_surface_event(pressed ? 1u : 0u, P3_XY[idx], pressed ? 127u : 0u);
    }
}

static void on_pad(int32_t velocity, int32_t pad, int32_t pressed, void* ctx)
{
    (void)ctx;
    uint32_t p = (uint32_t)pad + 43u;
    if (p < 43u + 64u) {
        uint16_t v = pressed ? (uint16_t)velocity : 0u;
        app_surface_event(pressed ? 1u : 0u, P3_XY[p], v);
    }
}

static void on_pad_pressure(int32_t pad, int32_t pressure, void* ctx)
{
    (void)ctx;
    uint32_t p = (uint32_t)pad + 43u;
    if (p < 43u + 64u) {
        app_aftertouch_event(P3_XY[p], (uint16_t)pressure);
    }
}

#define CONTROL_MAILBOX   (*(void**)0x2004fa6c)

typedef int32_t (*osMailGet_fn)(int32_t* evt_out, void* mail_id, uint32_t timeout);
typedef int32_t (*osMailFree_fn)(void* mail_id, void* mail);

#define OS_MAIL_GET   ((osMailGet_fn)(0x08015980u | 1u))
#define OS_MAIL_FREE  ((osMailFree_fn)(0x08015a18u | 1u))

static inline uint16_t rd_u16_le(uint8_t* p) {
    return (uint16_t)p[0] | ((uint16_t)p[1] << 8);
}

typedef int32_t (*osMailPut_fn)(void* mail_id, void* mail, uint32_t timeout);
#define OS_MAIL_PUT   ((osMailPut_fn)(0x08015916u | 1u))

static inline bool is_control_msg(uint8_t t) {
    return (t == 0x03) || (t == 0x0C) || (t == 0x0D);
}

static inline bool is_midi_msg(uint8_t t) {
    return (t == 0x06) || (t == 0x07);
}

void pump_controls_events(void) {
    void* mbox = CONTROL_MAILBOX;
    int32_t evt[3];

    /* Budget, damit du nicht die Welt pro Tick leerziehst */
    for (int i = 0; i < 32; i++) {
        OS_MAIL_GET(evt, mbox, 0);
        if (evt[0] != 0x20) break;

        uint8_t* msg = (uint8_t*)evt[1];
        if (!msg) break;

        uint8_t t = msg[0];

        /* WICHTIG: MIDI/SysEx NICHT konsumieren.
           Sofort zurück in die Mailbox und raus, damit OFW das abarbeitet. */
        if (is_midi_msg(t)) {
            if (OS_MAIL_PUT(mbox, msg, 0) != 0) {
                /* sollte nicht passieren, aber falls doch: nicht leak-en */
                OS_MAIL_FREE(mbox, msg);
            }
            return;
        }

        if (is_control_msg(t)) {
            switch (t) {
                case 0x03: {
                    uint8_t idx = msg[4];
                    uint8_t pressed = msg[5];
                    app_surface_event(pressed ? 1u : 0u, P3_XY[idx], pressed ? 127u : 0u);
                    break;
                }
                case 0x0C: {
                    uint8_t pad = msg[4];
                    uint8_t pressed = msg[5];
                    uint16_t val = rd_u16_le(&msg[6]);
                    uint8_t xyIndex = (uint8_t)(pad + 43);
                    app_surface_event(pressed ? 1u : 0u, P3_XY[xyIndex], pressed ? val : 0);
                    break;
                }
                case 0x0D: {
                    uint8_t pad = msg[4];
                    uint16_t val = rd_u16_le(&msg[6]);
                    uint8_t xyIndex = (uint8_t)(pad + 43);
                    app_aftertouch_event(P3_XY[xyIndex], val);
                    break;
                }
                default:
                    break;
            }

            OS_MAIL_FREE(mbox, msg);
            continue;
        }

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

static inline void cfw_runtime_init_once(void) {
    const uint32_t M0 = 0xC0DEF00Du;
    const uint32_t M1 = 0x385FFu;

    if (g_rt_magic0 == M0 && g_rt_magic1 == M1) return;

    uint32_t* src = &_sidata;
    uint32_t* dst = &_sdata;
    while (dst < &_edata) {
        *dst++ = *src++;
    }

    uint32_t* b = &_sbss;
    while (b < &_ebss) {
        *b++ = 0u;
    }

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
    if (!arg2 || arg3 <= 0) return 0;

    /* Only treat real SysEx as SysEx */
    const uint8_t b0 = (uint8_t)arg2[0];

    if (b0 == 0xF0) {
        /* Real SysEx */
        app_sysex_event(1u, (uint8_t*)arg2, (uint16_t)arg3);

        /* Critical: release the OFW SysEx semaphore (pointer is stored at 0x2004ea98) */
        void* sem = SYSEX_SEM;
        if (sem) {
            OS_SEM_GIVE(sem);
        }
        return 0;
    }

    if (arg1 != 4) return 0;          /* port 4 only */

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
    (void)data;
    (void)len;

    SEND_MIDI(4, data, len);
}

uint32_t driver_millis() {
    return ms;
}
