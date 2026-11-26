/* API-0011 / TC-0036: owned byte-array JNI translation, no engine algorithms.
 * JNIEnv and local references never leave this synchronous managed-thread call.
 */
#include <jni.h>
#include "hoyadb.h"
#include <stdlib.h>
#include <string.h>

#define HOYA_FRAME_LIMIT ((size_t)1048576)

struct collected {
  uint8_t *bytes;
  size_t length;
  unsigned int calls;
  int failed;
};

static void throw_static(JNIEnv *env, const char *kind, const char *message) {
  jclass type = (*env)->FindClass(env, kind);
  if (type != NULL) {
    (*env)->ThrowNew(env, type, message);
    (*env)->DeleteLocalRef(env, type);
  }
}

static void reply(void *context, const uint8_t *bytes, size_t length) {
  struct collected *result = context;
  result->calls++;
  if (result->calls != 1 || bytes == NULL || length == 0 || length > HOYA_FRAME_LIMIT) {
    result->failed = 1;
    return;
  }
  result->bytes = malloc(length);
  if (result->bytes == NULL) {
    result->failed = 1;
    return;
  }
  memcpy(result->bytes, bytes, length); /* Borrowed C span ends on return. */
  result->length = length;
}

static uint8_t *copy(JNIEnv *env, jbyteArray input, jsize length) {
  if (length == 0) return NULL;
  uint8_t *owned = malloc((size_t)length);
  if (owned == NULL) {
    throw_static(env, "java/lang/OutOfMemoryError", "NativeAllocationFailure");
    return NULL;
  }
  (*env)->GetByteArrayRegion(env, input, 0, length, (jbyte *)owned);
  if ((*env)->ExceptionCheck(env)) { free(owned); return NULL; }
  return owned;
}

static jbyteArray invoke(JNIEnv *env, jbyteArray input, jbyteArray key, int kind, jint validation_length) {
  const int opening = kind == 1;
  if (input == NULL || (opening && key == NULL)) {
    throw_static(env, "java/lang/IllegalArgumentException", "InvalidRequest");
    return NULL;
  }
  jsize length = (*env)->GetArrayLength(env, input);
  jsize key_length = opening ? (*env)->GetArrayLength(env, key) : (kind == 2 ? validation_length : 0);
  if (length < 0 || (size_t)length > HOYA_FRAME_LIMIT ||
      (key_length != 0 && key_length != 32)) {
    throw_static(env, "java/lang/IllegalArgumentException", "InvalidRequest");
    return NULL;
  }
  uint8_t *owned = copy(env, input, length);
  if ((*env)->ExceptionCheck(env)) return NULL;
  uint8_t *secret = opening ? copy(env, key, key_length) : NULL;
  if ((*env)->ExceptionCheck(env)) { free(owned); return NULL; }
  struct collected output = {0};
  int32_t status = kind == 2
      ? hoya_v1_validate_open(owned, (size_t)length, (size_t)key_length, reply, &output)
      : (opening ? hoya_v1_open(owned, (size_t)length, secret, (size_t)key_length, reply, &output)
                 : hoya_v1_call(owned, (size_t)length, reply, &output));
  /* The caller owns its byte[]; erase this temporary binary-key copy only. */
  if (secret != NULL) {
    volatile uint8_t *erase = secret;
    for (jsize i = 0; i < key_length; ++i) erase[i] = 0;
    free(secret);
  }
  free(owned);
  if (status != 0 || output.failed || output.calls != 1) {
    free(output.bytes);
    throw_static(env, status == 1 ? "java/lang/IllegalArgumentException" : "java/lang/IllegalStateException",
                 status == 1 ? "InvalidRequest" : "NativeFailure");
    return NULL;
  }
  jbyteArray result = (*env)->NewByteArray(env, (jsize)output.length);
  if (result != NULL) (*env)->SetByteArrayRegion(env, result, 0, (jsize)output.length, (const jbyte *)output.bytes);
  free(output.bytes);
  if ((*env)->ExceptionCheck(env)) return NULL;
  return result;
}

JNIEXPORT jint JNICALL Java_dev_hoyadb_NativeBridge_abiVersion(JNIEnv *env, jclass cls) {
  (void)env; (void)cls;
  return (jint)hoya_v1_abi_version();
}

JNIEXPORT jbyteArray JNICALL Java_dev_hoyadb_NativeBridge_open(JNIEnv *env, jclass cls, jbyteArray input, jbyteArray key) {
  (void)cls;
  return invoke(env, input, key, 1, 0);
}

JNIEXPORT jbyteArray JNICALL Java_dev_hoyadb_NativeBridge_call(JNIEnv *env, jclass cls, jbyteArray input) {
  (void)cls;
  return invoke(env, input, NULL, 0, 0);
}

JNIEXPORT jbyteArray JNICALL Java_dev_hoyadb_NativeBridge_validateOpen(JNIEnv *env, jclass cls, jbyteArray input, jint key_length) {
  (void)cls;
  return invoke(env, input, NULL, 2, key_length);
}
