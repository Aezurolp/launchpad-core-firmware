#include <stdint.h>
#include <stddef.h>
#include <string.h>

#define THUMB(addr) ((addr) | 1u)
#define FN(addr, type) ((type)(uintptr_t)THUMB(addr))

#define OFW_CREATE_THREAD_ADDR      0x080152deu
#define OFW_MAILBOX_CREATE_ADDR     0x0801571cu
#define OFW_M0TX_INIT_ORIG_ADDR     0x08025868u

#define OFW_M0TX_THREAD_HANDLE_PTR  0x2004fa28u
#define OFW_M0TX_MAILBOX_PTR        0x2004fa2cu

#define OFW_M0TX_THREADDEF_ADDR     0x08025c7cu
#define OFW_M0TX_MAILBOX_NAME_ADDR  0x08025c98u

#ifndef CFW_M0TX_MAIL_DEPTH
#define CFW_M0TX_MAIL_DEPTH  2u
#endif

#ifndef CFW_M0TX_PRIORITY
#define CFW_M0TX_PRIORITY    2u
#endif

typedef void*   (*ofw_create_thread_fn)(int32_t* threaddef, void* arg);
typedef void*   (*ofw_mailbox_create_fn)(const void* name_ptr, void* owner_thread, uint32_t depth);
typedef void    (*ofw_m0tx_init_orig_fn)(void);

#define OFW_CREATE_THREAD    FN(OFW_CREATE_THREAD_ADDR, ofw_create_thread_fn)
#define OFW_MAILBOX_CREATE   FN(OFW_MAILBOX_CREATE_ADDR, ofw_mailbox_create_fn)
#define OFW_M0TX_INIT_ORIG   FN(OFW_M0TX_INIT_ORIG_ADDR, ofw_m0tx_init_orig_fn)

__attribute__((section(".cfw_state"), aligned(4)))
static uint32_t g_m0tx_threaddef_copy[7];

__attribute__((section(".cfw_keep")))
void CFW_M0Tx_Init_Tuned(void)
{
    void* owner = *(void**)(uintptr_t)OFW_M0TX_THREAD_HANDLE_PTR; /* usually NULL at init */
    void* q = OFW_MAILBOX_CREATE((const void*)(uintptr_t)OFW_M0TX_MAILBOX_NAME_ADDR, owner, CFW_M0TX_MAIL_DEPTH);
    if (!q) {
        OFW_M0TX_INIT_ORIG();
        return;
    }
    *(void**)(uintptr_t)OFW_M0TX_MAILBOX_PTR = q;

    memcpy(g_m0tx_threaddef_copy, (const void*)(uintptr_t)OFW_M0TX_THREADDEF_ADDR, sizeof(g_m0tx_threaddef_copy));

    g_m0tx_threaddef_copy[2] = (uint32_t)CFW_M0TX_PRIORITY;

    void* th = OFW_CREATE_THREAD((int32_t*)g_m0tx_threaddef_copy, NULL);
    if (!th) {
        OFW_M0TX_INIT_ORIG();
        return;
    }

    *(void**)(uintptr_t)OFW_M0TX_THREAD_HANDLE_PTR = th;
}