#ifndef HOYADB_NATIVE_V1_H
#define HOYADB_NATIVE_V1_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* API-0011, ABI1. All nonempty admitted spans must be readable for the full
 * synchronous call. Null/nonzero or excessive lengths reject before dereference;
 * arbitrary invalid valid-size addresses are a caller safety violation.
 * The collector/context must remain valid and must not throw/unwind. Reply bytes
 * are borrowed only for the collector: copy them before returning. No application
 * handler or event callback runs inside this collector. The registry lock has
 * ended before collection. Rust panic-hook logging stays process-controlled.
 *
 * Status: 0 = exactly one complete JSON reply; 1 = invalid span/UTF8/collector;
 * 2 = contained Rust panic/poison/internal boundary failure. No raw panic text is
 * returned. Process aborts, foreign exceptions and invalid memory faults cannot
 * be contained by this contract. Link one facade runtime per application process.
 */
typedef void (*hoya_v1_reply)(void *context, const uint8_t *bytes, size_t length);

uint32_t hoya_v1_abi_version(void);

/* Keys never enter JSON. Protected opens supply exactly32 bytes. Memory supplies
 * (NULL,0). A key-length mismatch rejects before reading the key or touching any
 * filesystem; engine-specific key requirements are also validated by core.
 */
int32_t hoya_v1_open(const uint8_t *json, size_t length,
                     const uint8_t *key, size_t key_length,
                     hoya_v1_reply reply, void *context);

/* Pure original-frame preflight. key_length is only a size, never key bytes.
 * Returns the ordinary Null/error reply without registry/owner/provider/FS
 * initialization. Same span/status/collector contract as open. 0/32 lengths only.
 */
int32_t hoya_v1_validate_open(const uint8_t *json, size_t length,
                              size_t key_length, hoya_v1_reply reply, void *context);

/* Ordinary operations only: JSON open is rejected here. */
int32_t hoya_v1_call(const uint8_t *json, size_t length,
                     hoya_v1_reply reply, void *context);

#ifdef __cplusplus
}
#endif
#endif
