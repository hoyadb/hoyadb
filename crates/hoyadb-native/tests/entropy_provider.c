/* TC-0036 G17: real LTC provider, deterministic failed OS entropy, test binary only. */
#include "sqlite3.h"
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <sys/types.h>

int hoya_entropy_test_open(const char *path, int flags) {
  (void)path;
  (void)flags;
  errno = EIO;
  return -1;
}

ssize_t hoya_entropy_test_read(int fd, void *out, size_t length) {
  (void)fd; (void)out; (void)length;
  assert(!"failed open must never read");
  return -1;
}

int hoya_entropy_test_close(int fd) {
  (void)fd;
  assert(!"failed open must never close");
  return -1;
}

int main(int argc, char **argv) {
  assert(argc == 2);
  sqlite3 *db = NULL;
  int status = sqlite3_open_v2(argv[1], &db, SQLITE_OPEN_READWRITE, NULL);
  if (status == SQLITE_OK) {
    unsigned char key[32];
    for (int i = 0; i < 32; ++i) key[i] = (unsigned char)(i * 7);
    status = sqlite3_key(db, key, sizeof(key));
    if (status == SQLITE_OK) {
      sqlite3_stmt *query = NULL;
      status = sqlite3_prepare_v2(db, "SELECT label FROM sample", -1, &query, NULL);
      sqlite3_finalize(query);
    }
  }
  assert(status != SQLITE_OK);
  if (db != NULL) assert(sqlite3_close(db) == SQLITE_OK);
  puts("REAL_PROVIDER_ENTROPY_FAILURE_CLOSED_GREEN");
  return 0;
}
