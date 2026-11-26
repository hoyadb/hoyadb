# Phase 7 C/JNI foundation checkpoint

The native ABI1 boundary delegates to the general Rust facade with owned replies,
separate binary keys and static errors. It adds no database algorithm or runtime
dependency. Root inspected the C header, Rust boundary and JNI copies, and
independently ran all nine native tests: three unit and six integration cases,
including real protected reopen and wrong-key byte preservation.

The additive readonly open preflight uses the exact original parser without
initializing the process registry or touching filesystem/provider policy. Invalid
lengths/null spans/UTF8 reject before dereference; admitted readable pointers and
collector lifetime remain caller obligations. The registry lock ends before the
reply collector; tests exercise safe reentry. Panic containment does not replace
application hooks or promise recovery from arbitrary foreign memory faults.

[Source/lock evidence](source-and-locks.json) records the frozen20-file foundation,
the actual tested source hashes, default-layout staged Rust hashes and the35
external package identities remaining within the approved graph. Separately
owned formatting work remains untouched. Native preflight and JNI test outputs
preserve authentic behavioral RED/GREEN; failed setup is not counted as a RED.

The compiled JNI stub test passes under checked JNI, including original-frame
validation, owned replies, eight static-error cases and32 multithread calls.
Refreshed Android x86_64 and arm64 JNI builds match their exact input/output
hashes and eight-export allowlist. These build manifests explicitly retain
`runtime_verified:false`; later Kotlin/RN target execution is separate evidence.
The earlier actual provider/entropy checks and Claude security review remain
supporting evidence in the parent Android crypto report, not a substituted
final mobile SDK test.

The entropy shim's nine fault cases retain the OS-only fail-closed contract;
production builds exclude the test injection macro. The byte bridge and host
wrappers cooperate with API-0011's shared payload-free terminal-release broker.
This foundation does not itself implement a second admission queue or invoke
application handlers in a C collector. Swift/Kotlin/RN broker, local consumer,
packaging and final cross-review gates remain open. No publication or user-store
operation is implied by this checkpoint.
