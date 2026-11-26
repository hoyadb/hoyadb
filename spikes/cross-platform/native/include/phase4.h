#ifndef HOYA_PHASE4_H
#define HOYA_PHASE4_H
#include <stddef.h>
#include <stdint.h>
/* API-0008: spans are borrowed only for this call/callback. Copy the reply.
   Callback must not unwind. Calls are serialized by the fixture; no lock spans
   reply. 0=delivered JSON, 1=invalid input, 2=panic/poison containment. */
typedef void (*hoya_command_reply)(void *context, const uint8_t *bytes, size_t length);
int32_t hoya_command_rpc(const uint8_t *request, size_t length,
    hoya_command_reply reply, void *context);
#endif
