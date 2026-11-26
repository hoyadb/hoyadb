/* TC-0036 G17: real target provider profile and independent-process ciphertext. */
#include "sqlite3.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void text(sqlite3 *db, const char *sql, const char *expected) {
  sqlite3_stmt *statement = NULL;
  assert(sqlite3_prepare_v2(db, sql, -1, &statement, NULL) == SQLITE_OK);
  assert(sqlite3_step(statement) == SQLITE_ROW);
  const unsigned char *value = sqlite3_column_text(statement, 0);
  assert(value != NULL && strcmp((const char *)value, expected) == 0);
  assert(sqlite3_step(statement) == SQLITE_DONE);
  assert(sqlite3_finalize(statement) == SQLITE_OK);
}

int main(int argc, char **argv) {
  assert(argc == 3);
  assert(strcmp(sqlite3_libversion(), "3.53.4") == 0);
  unsigned char key[32];
  for (int i = 0; i < 32; i++) key[i] = (unsigned char)(i * 7);
  sqlite3 *db = NULL;
  assert(sqlite3_open_v2(argv[2], &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, NULL) == SQLITE_OK);
  if (strcmp(argv[1], "missing") != 0) {
    if (strcmp(argv[1], "wrong") == 0) key[0] ^= 0x80;
    assert(sqlite3_key(db, key, sizeof(key)) == SQLITE_OK);
  }
  if (strcmp(argv[1], "wrong") == 0 || strcmp(argv[1], "missing") == 0) {
    sqlite3_stmt *statement = NULL;
    assert(sqlite3_prepare_v2(db, "SELECT name FROM sqlite_schema", -1, &statement, NULL) != SQLITE_OK);
    sqlite3_finalize(statement);
    assert(sqlite3_close(db) == SQLITE_OK);
    puts("PROVIDER_FAIL_CLOSED_GREEN");
    return 0;
  }
  text(db, "PRAGMA cipher_version", "4.19.0 community");
  text(db, "PRAGMA cipher_provider", "libtomcrypt");
  text(db, "PRAGMA cipher_provider_version", "1.18.2");
  text(db, "PRAGMA cipher_page_size", "4096");
  text(db, "PRAGMA cipher_plaintext_header_size", "0");
  text(db, "PRAGMA cipher_hmac_algorithm", "HMAC_SHA512");
  text(db, "PRAGMA cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512");
  text(db, "PRAGMA temp_store", "0"); /* Compile-time TEMP_STORE=3 still forces memory. */
  assert(sqlite3_threadsafe() == 1);
  if (strcmp(argv[1], "writer") == 0) {
    assert(sqlite3_exec(db, "CREATE TABLE sample(label TEXT NOT NULL,n INTEGER NOT NULL);"
      "INSERT INTO sample VALUES('HOYA_ANDROID_CIPHER_CANARY_λ',9223372036854775807);", NULL, NULL, NULL) == SQLITE_OK);
  } else {
    assert(strcmp(argv[1], "reader") == 0);
  }
  text(db, "SELECT label FROM sample", "HOYA_ANDROID_CIPHER_CANARY_λ");
  text(db, "SELECT CAST(n AS TEXT) FROM sample", "9223372036854775807");
  assert(sqlite3_close(db) == SQLITE_OK);
  puts("PROVIDER_PROFILE_AND_DATA_GREEN SQLCipher=4.19.0 SQLite=3.53.4 provider=LibTomCrypt1.18.2");
  return 0;
}
