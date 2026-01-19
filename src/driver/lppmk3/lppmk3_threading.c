#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include "lppmk3_threading.h"

#define THUMB(addr) ((addr) | 1u)
#define FN(addr, type) ((type)(uintptr_t)THUMB(addr))

#define OFW_CREATE_THREAD_ADDR      0x080152deu
#define OFW_MAILBOX_CREATE_ADDR     0x0801571cu
#define OFW_M0TX_INIT_ORIG_ADDR     0x08025868u
#define OFW_SET_PRIO_ADDR           0x0801534eu

#define OFW_M0TX_THREAD_HANDLE_PTR  0x2004fa28u
#define OFW_M0TX_MAILBOX_PTR        0x2004fa2cu

#define OFW_M0TX_THREADDEF_ADDR     0x08025c7cu
#define OFW_M0TX_MAILBOX_NAME_ADDR  0x08025c98u

#ifndef CFW_M0TX_MAIL_DEPTH
#define CFW_M0TX_MAIL_DEPTH  8u
#endif

#ifndef CFW_M0TX_PRIORITY_INIT
#define CFW_M0TX_PRIORITY_INIT  0u
#endif

#ifndef CFW_M0TX_PRIORITY_BOOST
#define CFW_M0TX_PRIORITY_BOOST 2u
#endif

typedef void*   (*ofw_create_thread_fn)(int32_t* threaddef, void* arg);
typedef void*   (*ofw_mailbox_create_fn)(const void* name_ptr, void* owner_thread, uint32_t depth);
typedef void    (*ofw_m0tx_init_orig_fn)(void);
typedef int32_t (*ofw_set_prio_fn)(uint32_t thread_id, uint32_t prio);

#define OFW_CREATE_THREAD    FN(OFW_CREATE_THREAD_ADDR, ofw_create_thread_fn)
#define OFW_MAILBOX_CREATE   FN(OFW_MAILBOX_CREATE_ADDR, ofw_mailbox_create_fn)
#define OFW_M0TX_INIT_ORIG   FN(OFW_M0TX_INIT_ORIG_ADDR, ofw_m0tx_init_orig_fn)
#define OFW_SET_PRIO         FN(OFW_SET_PRIO_ADDR, ofw_set_prio_fn)

static inline uint32_t m0tx_thread_id(void) {
    return *(volatile uint32_t*)(uintptr_t)OFW_M0TX_THREAD_HANDLE_PTR;
}

__attribute__((section(".cfw_state"), aligned(4)))
static uint32_t g_m0tx_threaddef_copy[7];

__attribute__((section(".cfw_keep"), used))
void boost_m0(void)
{
    uint32_t tid = m0tx_thread_id();
    if (!tid) return;
    OFW_SET_PRIO(tid, (uint32_t)CFW_M0TX_PRIORITY_BOOST);
}

__attribute__((section(".cfw_keep"), used))
void unboost_m0(void)
{
    uint32_t tid = m0tx_thread_id();
    if (!tid) return;
    OFW_SET_PRIO(tid, (uint32_t)CFW_M0TX_PRIORITY_INIT);
}

__attribute__((section(".cfw_keep")))
void CFW_M0Tx_Init_Tuned(void)
{
    void* owner = *(void**)(uintptr_t)OFW_M0TX_THREAD_HANDLE_PTR;
    void* q = OFW_MAILBOX_CREATE((const void*)(uintptr_t)OFW_M0TX_MAILBOX_NAME_ADDR, owner, CFW_M0TX_MAIL_DEPTH);
    if (!q) {
        OFW_M0TX_INIT_ORIG();
        return;
    }
    *(void**)(uintptr_t)OFW_M0TX_MAILBOX_PTR = q;

    memcpy(g_m0tx_threaddef_copy, (const void*)(uintptr_t)OFW_M0TX_THREADDEF_ADDR, sizeof(g_m0tx_threaddef_copy));

    g_m0tx_threaddef_copy[2] = (uint32_t)CFW_M0TX_PRIORITY_INIT;

    void* th = OFW_CREATE_THREAD((int32_t*)g_m0tx_threaddef_copy, NULL);
    if (!th) {
        OFW_M0TX_INIT_ORIG();
        return;
    }

    *(void**)(uintptr_t)OFW_M0TX_THREAD_HANDLE_PTR = th;
}
