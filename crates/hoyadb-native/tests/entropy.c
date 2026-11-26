/* TC-0036 G17: deterministic faults at the OS boundary, no cipher substitute. */
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>

unsigned long rng_get_bytes(unsigned char *, unsigned long, void (*)(void));
static int mode, opens, reads, closes, callbacks;

int hoya_entropy_test_open(const char *path, int flags) {
  assert(strcmp(path, "/dev/urandom") == 0);
  assert((flags & O_ACCMODE) == O_RDONLY);
  assert((flags & O_CLOEXEC) != 0);
  opens++;
  if (mode == 1) { errno = EACCES; return -1; }
  if (mode == 2 && opens == 1) { errno = EINTR; return -1; }
  return 0; /* Descriptor zero is valid. */
}

ssize_t hoya_entropy_test_read(int fd, void *buffer, size_t length) {
  assert(fd == 0);
  reads++;
  if (mode == 3 && reads == 1) { errno = EINTR; return -1; }
  if ((mode == 4 || mode == 5) && reads == 2) {
    if (mode == 5) { errno = EIO; return -1; }
    return 0;
  }
  size_t count = reads == 1 && length > 2 ? 2 : length;
  memset(buffer, 0x6d, count);
  return (ssize_t)count;
}

int hoya_entropy_test_close(int fd) {
  assert(fd == 0);
  closes++;
  if (mode == 6) { errno = EINTR; return -1; }
  return 0;
}

static void callback(void) { callbacks++; }

static void reset(int next) {
  mode = next;
  opens = reads = closes = callbacks = 0;
}

static void check(int selected, int succeeds) {
  reset(selected);
  unsigned char output[32];
  memset(output, 0xa3, sizeof(output));
  assert(rng_get_bytes(output, sizeof(output), callback) ==
         (succeeds ? sizeof(output) : 0));
  for (size_t i = 0; i < sizeof(output); ++i)
    assert(output[i] == (succeeds ? 0x6d : 0));
  assert(callbacks == 0); /* Never invokes a clock-entropy callback. */
  assert(closes == (selected == 1 ? 0 : 1));
}

int main(void) {
  check(0, 1); /* partial read then complete, including descriptor zero */
  check(1, 0); /* open failure clears entire caller buffer */
  check(2, 1); /* open EINTR retry */
  assert(opens == 2);
  check(3, 1); /* read EINTR retry */
  assert(reads == 2);
  check(4, 0); /* EOF after partial entropy clears prefix and suffix */
  check(5, 0); /* EIO after partial entropy clears entire output */
  check(6, 0); /* close EINTR closes once: no reused descriptor retry */
  reset(0);
  assert(rng_get_bytes(NULL, 32, callback) == 0);
  assert(rng_get_bytes(NULL, 0, callback) == 0);
  assert(opens == 0 && closes == 0 && callbacks == 0);
  puts("HOYA_ENTROPY_FAULTS_GREEN cases=9");
  return 0;
}
