The first verify_keyid.py run fetched the gson key files and then crashed with UnicodeDecodeError while
decoding gpg's colon listing of a key whose user ID is not valid UTF-8 (Éamonn McManus). No verification
result was recorded by that run. The script was changed only to decode command output with
errors='replace' for logging; the rerun refetched every key (overwriting the same paths) in a fresh owned
home and produced verification-keyid-fallback.json. Signature verification itself uses gpg exit/status
lines and was unaffected.
