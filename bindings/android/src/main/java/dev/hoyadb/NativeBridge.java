package dev.hoyadb;

/** Internal owned byte-array transport; all database validation remains in Rust. */
final class NativeBridge {
  static { System.loadLibrary("hoyadb_jni"); }
  private NativeBridge() {}
  static native int abiVersion();
  static native byte[] validateOpen(byte[] input, int keyLength);
  static native byte[] open(byte[] frame, byte[] key);
  static native byte[] call(byte[] frame);
}
