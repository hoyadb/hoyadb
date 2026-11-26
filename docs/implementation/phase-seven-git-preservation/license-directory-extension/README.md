# Raw captured license directory preservation

DEF-0027's extension was specified at `523e394` and approved by the focused
Claude Opus 5.5 prep review before implementation.

G28 now enumerates directories named exactly `licenses` anywhere beneath
`docs/implementation`, matching the scoped attribute. The regression explicitly
requires the historical JDOM capture and at least one CRLF-bearing document
license, avoiding vacuous success. Deliberate iOS `component-licenses`
presentation copies remain outside this class.

Before the attribute change, the test failed 23 real filter comparisons:
2 with `input` and 21 with `true`, with no harness errors. The historical JDOM
file failed both clean and checkout under `input`. After widening the scoped
`-text` pattern, the complete three-mode test passes normally and under `-O`,
including the ordinary-source conversion control.

`results.json` records the before/after source hashes and original-to-derived
log hashes. Log copies replace only the absolute repository prefix with
`<repo>`; the original logs remain under the named `target/` directory.

No runtime, package, frozen evidence index or user Git setting was changed.
Whitespace diagnostics remain enabled. The JDOM 2 license is a historical
research capture, not attribution for Kotlin's IntelliJ JDOM fork.
