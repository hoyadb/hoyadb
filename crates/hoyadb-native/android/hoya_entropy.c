/* API-0011 / TC-0036 G17: public Android API24 OS entropy only.
 * This unit replaces upstream LibTomCrypt rng_get_bytes.c, which must not
 * also be linked. No entropy hook, clock dispatcher or weak fallback exists.
 */
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/types.h>
#include <unistd.h>

#ifdef HOYA_ENTROPY_TEST
extern int hoya_entropy_test_open(const char *, int);
extern ssize_t hoya_entropy_test_read(int, void *, size_t);
extern int hoya_entropy_test_close(int);
#define entropy_open hoya_entropy_test_open
#define entropy_read hoya_entropy_test_read
#define entropy_close hoya_entropy_test_close
#else
#define entropy_open open
#define entropy_read read
#define entropy_close close
#endif

_Static_assert(sizeof(unsigned long) <= sizeof(size_t), "entropy size mismatch");

static void clear_output(unsigned char *out, unsigned long length) {
  volatile unsigned char *cursor = out;
  while (length-- != 0) *cursor++ = 0;
}

unsigned long rng_get_bytes(unsigned char *out, unsigned long length,
                           void (*callback)(void)) {
  /* The callback belongs to upstream's clock fallback, which is forbidden. */
  (void)callback;
  if (out == NULL || length == 0) return 0;
  int fd;
  do {
    fd = entropy_open("/dev/urandom", O_RDONLY | O_CLOEXEC);
  } while (fd < 0 && errno == EINTR);
  if (fd < 0) {
    clear_output(out, length);
    return 0;
  }
  unsigned long filled = 0;
  while (filled < length) {
    size_t remaining = (size_t)(length - filled);
    size_t count = remaining > (size_t)SSIZE_MAX ? (size_t)SSIZE_MAX : remaining;
    ssize_t received = entropy_read(fd, out + filled, count);
    if (received < 0 && errno == EINTR) continue;
    if (received <= 0 || (size_t)received > count) break;
    filled += (unsigned long)received;
  }
  /* Never retry close: EINTR can already have closed/reused the descriptor. */
  int closed = entropy_close(fd);
  if (filled != length || closed != 0) {
    clear_output(out, length);
    return 0;
  }
  return length;
}
