/* TC-0036 G16 / Android crypto packet case8: closed synthetic raw-key fixture.
 * No SDK import facility: this executable only touches explicit task-owned paths.
 */
#include "sqlite3.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void text(sqlite3 *db, const char *query, const char *expected) {
    sqlite3_stmt *statement = NULL;
    assert(sqlite3_prepare_v2(db, query, -1, &statement, NULL) == SQLITE_OK);
    assert(sqlite3_step(statement) == SQLITE_ROW);
    const unsigned char *value = sqlite3_column_text(statement, 0);
    assert(value && strcmp((const char *)value, expected) == 0);
    assert(sqlite3_step(statement) == SQLITE_DONE);
    assert(sqlite3_finalize(statement) == SQLITE_OK);
}

int main(int argc, char **argv) {
    assert(argc == 4);
    const int writer = strcmp(argv[1], "writer") == 0;
    const int update = strcmp(argv[1], "update") == 0;
    assert(writer || update || strcmp(argv[1], "reader") == 0);
    assert(strcmp(sqlite3_libversion(), "3.53.4") == 0);
    sqlite3 *db = NULL;
    assert(sqlite3_open_v2(argv[2], &db, SQLITE_OPEN_READWRITE | (writer ? SQLITE_OPEN_CREATE : 0), NULL) == SQLITE_OK);
    /* Exact supplied raw32-byte policy, not a password or implicit KDF. Public
     * fixed synthetic bytes never enter argv, logs or a JSON frame. */
    unsigned char key[32];
    char sql[96] = "PRAGMA key=\"x'";
    static const char hex[] = "0123456789abcdef";
    size_t offset = strlen(sql);
    for (int i = 0; i < 32; i++) {
        key[i] = (unsigned char)(i * 7);
        sql[offset++] = hex[key[i] >> 4];
        sql[offset++] = hex[key[i] & 15];
    }
    memcpy(sql + offset, "'\"", 3);
    assert(sqlite3_exec(db, sql, NULL, NULL, NULL) == SQLITE_OK);
    volatile unsigned char *secret = key;
    for (size_t i = 0; i < sizeof(key); i++) secret[i] = 0;
    volatile char *frame = sql;
    for (size_t i = 0; i < sizeof(sql); i++) frame[i] = 0;
    text(db, "PRAGMA cipher_version", "4.19.0 community");
    text(db, "PRAGMA cipher_provider", argv[3]);
    text(db, "PRAGMA cipher_page_size", "4096");
    text(db, "PRAGMA cipher_plaintext_header_size", "0");
    text(db, "PRAGMA cipher_hmac_algorithm", "HMAC_SHA512");
    text(db, "PRAGMA cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512");
    assert(sqlite3_threadsafe() == 1);
    if (writer) {
        assert(sqlite3_exec(db, "CREATE TABLE sample(label TEXT NOT NULL,n INTEGER NOT NULL);"
            "INSERT INTO sample VALUES('HOYA_CROSS_PROVIDER_東京',9223372036854775807);", NULL, NULL, NULL) == SQLITE_OK);
    }
    if (writer || update) {
        text(db, "SELECT label FROM sample", "HOYA_CROSS_PROVIDER_東京");
        text(db, "SELECT CAST(n AS TEXT) FROM sample", "9223372036854775807");
    }
    if (update) {
        assert(sqlite3_exec(db, "UPDATE sample SET label='HOYA_CROSS_PROVIDER_返',n=-9223372036854775808;", NULL, NULL, NULL) == SQLITE_OK);
    }
    if (!writer) {
        text(db, "SELECT label FROM sample", "HOYA_CROSS_PROVIDER_返");
        text(db, "SELECT CAST(n AS TEXT) FROM sample", "-9223372036854775808");
    }
    assert(sqlite3_close(db) == SQLITE_OK);
    puts("HOYA_CROSS_PROVIDER_PROFILE_DATA_GREEN SQLCipher=4.19.0 SQLite=3.53.4 raw-key32");
    return 0;
}
