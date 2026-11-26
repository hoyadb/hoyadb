# Manual consumer tooling acceptance before activation

This source-prepared harness is not yet executed. Existing guards were written
before these tooling tests; their future passes are not retrospectively TDD REDs.
Absent failure-evidence behavior must first fail under the following real tests
before its correction. No subprocess/test/emulator runs during the benchmark
quiet window. Test subprocesses are mocked; no synthetic invalid artifact is
passed to a real compiler/device.

- C01: consumer project must be fresh, outside checkout and under the owned temp
  prefix. Reject inside/existing destinations before any tool activation.
- C02: changed local AAR/POM or approved runtime archive hash rejects before
  compiler/D8/signing. Unreviewed Java injection rejects; no ignored assertion
  guard under Python -O. No installation/download/cache activation.
- C03: actual wrong device page size or malformed/foreign app identity rejects
  before install. Persist static failed-phase/command/exit-code evidence even
  when validation, earlier adb or install fails; never pretend it ran.
- C04: failed fixture execution preserves its JSON and the exact owned app/store
  for diagnosis, force-stops only that owned app, and never uninstalls because a
  success cleanup flag was supplied. Record actual/unknown installation state.
- C05: successful full+seed+fresh-process restart evidence checks nonempty exact
  cases, all pass,16384 pages and distinct seed/restart PIDs. Only a fully
  successful owned run may uninstall its app when requested.
  The predeclared identifiers/order are consumer/expected-cases.json (21 full,
  one seed, one restart). An all-pass report containing only a shortened subset
  must first demonstrate the current controller gap before its correction; no
  aggregate count inferred from arbitrary returned cases certifies this matrix.
- C06: actual local AAR independent compile uses no SDK source paths; complete
  APK D8 program inputs, two native slices, stored JNI ZIP entries, verified ELF
  and ZIP16KiB, actual owned emulator execution and static bounded reports.
  This is the final runtime gate, not a mock's substitute.
