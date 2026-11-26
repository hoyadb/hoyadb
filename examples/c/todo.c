/* A Todo app over the HoyaDB native C ABI (ABI1). The C ABI is a framed JSON interface: every
 * operation sends one JSON frame and receives one JSON reply. It is not a fluent database API;
 * Swift, Kotlin and React Native wrap these same frames. This file is compiled and executed by the
 * documentation build against a locally built libhoyadb_native.a. Named regions are the snippets
 * on the C getting-started page. To keep it short, frames are built with snprintf from values the
 * core itself returned (plain ASCII handles and keys), and replies are read with string searches.
 * Those helpers only handle these known fixtures; a real application should use a JSON library. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "hoyadb.h"

#define CHECK(condition, message) \
  do { if (!(condition)) { fprintf(stderr, "check failed: %s\n", message); exit(1); } } while (0)

// docs:start reply
/* Reply bytes are borrowed: they are valid only while the collector runs. Copy them into memory
 * the application owns; the caller frees that copy. The library never hands out memory to free. */
typedef struct { char *text; } Reply;

/* The collector has the hoya_v1_reply signature. */
static void collect(void *context, const uint8_t *bytes, size_t length) {
  Reply *reply = context;
  reply->text = malloc(length + 1);
  if (reply->text) { memcpy(reply->text, bytes, length); reply->text[length] = '\0'; }
}

#define OK_PREFIX "{\"abi\":\"1\",\"ok\":true"
static int ok(const char *reply) { return strncmp(reply, OK_PREFIX, sizeof OK_PREFIX - 1) == 0; }

/* Sends one frame and requires a successful reply. Returns an owned copy the caller must free(). */
static char *call(const char *frame) {
  Reply reply = {NULL};
  int32_t status = hoya_v1_call((const uint8_t *)frame, strlen(frame), collect, &reply);
  CHECK(status == 0 && reply.text, "the ABI returned one reply");
  if (!ok(reply.text)) { fprintf(stderr, "error reply: %s\n", reply.text); exit(1); }
  return reply.text;
}
// docs:end reply

/* Copies the string value that follows `marker` in a reply, e.g. "\"value\":\"". */
static char *string_after(const char *text, const char *marker) {
  const char *start = strstr(text, marker);
  CHECK(start, marker);
  start += strlen(marker);
  const char *end = strchr(start, '"');
  CHECK(end, "a terminated string");
  char *value = malloc((size_t)(end - start) + 1);
  CHECK(value, "memory for the copied string");
  memcpy(value, start, (size_t)(end - start));
  value[end - start] = '\0';
  return value;
}

int main(void) {
  char frame[4096];

  // docs:start open
  CHECK(hoya_v1_abi_version() == 1, "the library speaks ABI1");
  /* One frame opens the owner: engine, schema, migrations, namespace and options. Memory storage
   * takes no key, so the key span is (NULL, 0). A protected store takes exactly 32 key bytes. */
  const char *open_frame =
      "{\"abi\":\"1\",\"op\":\"open\",\"owner\":null,\"args\":{"
      "\"engine\":{\"kind\":\"memory\",\"store\":null},"
      "\"schema\":{\"version\":\"1\",\"tables\":{\"todos\":{\"columns\":{"
      "\"title\":{\"kind\":\"String\",\"optional\":false},"
      "\"done\":{\"kind\":\"Boolean\",\"optional\":false}},\"relations\":{}}}},"
      "\"migrations\":[],\"namespace\":\"0123456789abcdef0123456789abcdef\","
      "\"options\":{\"capacity\":\"1000\",\"history\":\"100\",\"initial_delay_ms\":\"1000\","
      "\"multiplier\":\"2\",\"max_delay_ms\":\"60000\"}}}";
  Reply opened = {NULL};
  int32_t status = hoya_v1_open((const uint8_t *)open_frame, strlen(open_frame), NULL, 0, collect, &opened);
  CHECK(status == 0 && opened.text && ok(opened.text), "the database opened");
  char *owner = string_after(opened.text, "\"value\":\"");
  free(opened.text);
  // docs:end open

  // docs:start query
  /* Values are tagged: {"kind":"boolean","value":false}. Predicates use Eq, Ne, Lt, Le, Gt, Ge. */
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"fetch\",\"owner\":\"%s\",\"args\":{\"query\":{\"table\":\"todos\","
           "\"predicates\":[{\"field\":\"done\",\"operator\":\"Eq\",\"values\":[{\"kind\":\"boolean\",\"value\":false}]}],"
           "\"order\":{\"field\":\"title\",\"direction\":\"Asc\"},\"limit\":null,\"source\":null}}}",
           owner);
  char *rows = call(frame);
  // rows: {"abi":"1","ok":true,"value":[]} until a task exists
  // docs:end query
  CHECK(strstr(rows, "\"value\":[]"), "an empty table returns no rows");
  free(rows);

  // docs:start observe
  /* Subscriptions deliver nothing by themselves: the host drains events when it chooses. */
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"observe\",\"owner\":\"%s\",\"args\":{\"query\":{\"table\":\"todos\","
           "\"predicates\":[],\"order\":null,\"limit\":null,\"source\":null},\"mode\":\"membership\",\"columns\":[]}}",
           owner);
  char *observed = call(frame);
  char *subscription = string_after(observed, "\"value\":\"");
  free(observed);
  snprintf(frame, sizeof frame, "{\"abi\":\"1\",\"op\":\"drain\",\"owner\":\"%s\",\"args\":{\"now_ms\":\"0\"}}", owner);
  char *events = call(frame);
  // events: the initial snapshot for the subscription, with "rows":[]
  // docs:end observe
  CHECK(strstr(events, subscription) && strstr(events, "\"rows\":[]"), "the initial snapshot is delivered");
  free(events);

  // docs:start command
  /* A command is a request ticket, a begin, scoped writes, then finish. The local handler is
   * simply the C code that runs between begin and finish. It proposes the created row's ID, which
   * must start with "local:" until the server confirms it. */
  snprintf(frame, sizeof frame, "{\"abi\":\"1\",\"op\":\"request\",\"owner\":\"%s\",\"args\":{}}", owner);
  char *ticket_reply = call(frame);
  char *ticket = string_after(ticket_reply, "\"value\":\"");
  free(ticket_reply);
  // docs:end command

  // docs:start execute
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"begin\",\"owner\":\"%s\",\"args\":{\"request\":\"%s\","
           "\"definition\":{\"name\":\"add-task-v1\",\"arguments\":{\"title\":{\"kind\":\"String\",\"optional\":false}},"
           "\"reference_paths\":[]},"
           "\"value\":{\"kind\":\"object\",\"value\":{\"title\":{\"kind\":\"string\",\"value\":\"Buy milk\"}}},"
           "\"context\":{\"identity\":{\"kind\":\"string\",\"value\":\"ada\"},\"now_ms\":\"1000\"}}}",
           owner, ticket);
  char *begun = call(frame);
  CHECK(strstr(begun, "\"value\":true"), "the command was admitted");
  free(begun);
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"scope_insert\",\"owner\":\"%s\",\"args\":{\"request\":\"%s\",\"table\":\"todos\","
           "\"fields\":{\"title\":{\"kind\":\"string\",\"value\":\"Buy milk\"},\"done\":{\"kind\":\"boolean\",\"value\":false}},"
           "\"proposed_id\":\"local:task-1\"}}",
           owner, ticket);
  free(call(frame));
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"finish\",\"owner\":\"%s\",\"args\":{\"request\":\"%s\","
           "\"result\":{\"kind\":\"object\",\"value\":{\"id\":{\"kind\":\"string\",\"value\":\"local:task-1\"}}}}}",
           owner, ticket);
  char *finished = call(frame);
  char *command_key = string_after(finished, "\"key\":\"");
  // The task is now stored locally as local:task-1 and the command is queued for sync.
  // docs:end execute
  free(finished);
  free(ticket);
  snprintf(frame, sizeof frame, "{\"abi\":\"1\",\"op\":\"drain\",\"owner\":\"%s\",\"args\":{\"now_ms\":\"1\"}}", owner);
  events = call(frame);
  CHECK(strstr(events, "local:task-1"), "the observer sees the optimistic task");
  free(events);

  // docs:start sync
  /* prepare_push freezes waiting work into envelopes. Send them to your server however you like;
   * here a tiny C "backend" answers with an outcome carrying the server's ID for the new task. */
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"prepare_push\",\"owner\":\"%s\",\"args\":{\"now_ms\":\"1000\",\"jitter_basis_points\":\"0\"}}",
           owner);
  char *batch = call(frame);
  CHECK(strstr(batch, "add-task-v1") && strstr(batch, command_key), "the envelope names the command and its key");
  free(batch);
  /* The backend reply body is ordinary JSON, embedded as an escaped string in the frame. */
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"receive_wire\",\"owner\":\"%s\",\"args\":{\"body\":"
           "\"[{\\\"idempotencyKey\\\":\\\"%s\\\",\\\"status\\\":\\\"applied\\\","
           "\\\"result\\\":{\\\"id\\\":\\\"server-task-1\\\"}}]\"}}",
           owner, command_key);
  free(call(frame));
  /* Reconcile: map the one local ID this command created to the server's ID. */
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"reconcile\",\"owner\":\"%s\",\"args\":{\"key\":\"%s\","
           "\"plan\":{\"writes\":[],\"mappings\":{\"local:task-1\":\"server-task-1\"}}}}",
           owner, command_key);
  free(call(frame)); /* call() has already checked that the reply is ok */
  // docs:end sync
  free(command_key);
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"fetch\",\"owner\":\"%s\",\"args\":{\"query\":{\"table\":\"todos\","
           "\"predicates\":[],\"order\":null,\"limit\":null,\"source\":null}}}",
           owner);
  rows = call(frame);
  CHECK(strstr(rows, "server-task-1") && !strstr(rows, "local:task-1"), "the task now has the server ID");
  free(rows);

  // docs:start close
  snprintf(frame, sizeof frame,
           "{\"abi\":\"1\",\"op\":\"unsubscribe\",\"owner\":\"%s\",\"args\":{\"subscription\":\"%s\"}}", owner, subscription);
  free(call(frame));
  snprintf(frame, sizeof frame, "{\"abi\":\"1\",\"op\":\"close\",\"owner\":\"%s\",\"args\":{}}", owner);
  free(call(frame));
  free(subscription);
  free(owner);
  // docs:end close
  puts("c tour: ok");
  return 0;
}
