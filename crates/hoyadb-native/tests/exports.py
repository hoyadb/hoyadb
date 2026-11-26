#!/usr/bin/env python3
"""TC-0036 G12: Android JNI package exports only its versioned boundary."""
import subprocess
import sys

allowed = {
    "hoya_v1_abi_version", "hoya_v1_open", "hoya_v1_call", "hoya_v1_validate_open",
    "Java_dev_hoyadb_NativeBridge_abiVersion", "Java_dev_hoyadb_NativeBridge_open",
    "Java_dev_hoyadb_NativeBridge_call", "Java_dev_hoyadb_NativeBridge_validateOpen",
}
output = subprocess.check_output([sys.argv[1], "--dynamic", "--defined-only", "--extern-only", sys.argv[2]], text=True)
symbols = {line.split()[-1].split("@")[0] for line in output.splitlines() if line.split()}
symbols.discard("HOYADB_1")
assert symbols == allowed, {"unexpected_count": len(symbols - allowed), "unexpected_sample": sorted(symbols - allowed)[:8], "missing": sorted(allowed - symbols)}
print("HOYA_ANDROID_EXPORTS_GREEN exports=8")
