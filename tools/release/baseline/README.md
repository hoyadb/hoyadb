# Public API compatibility baseline

The JSON API dumps, declaration file and snapshot manifest are immutable inputs
to [compatibility.py](../compatibility.py). They retain the exact bytes and hashes
used by the compatibility gate. Moving them does not establish a new runtime
result or authorize a baseline change. Source drift and fresh Swift comparisons
continue to use the existing checks and separately reviewed accepted changes.
