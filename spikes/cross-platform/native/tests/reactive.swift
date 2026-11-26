// SPEC-0027 / API-0005 / TC-0029 R14–R15: production-core native delivery.
import Darwin
import Dispatch
import Foundation

enum Failure: Error { case check(String) }
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw Failure.check(message) }
}
struct Row: Equatable {
    let id: String
    let status: String
    let notes: String
    let total: Int64
}
func string(_ bytes: UnsafePointer<UInt8>?, _ count: Int) -> String {
    String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
}
func copyRows(_ rows: UnsafePointer<hoya_reactive_row>?, _ count: Int) -> [Row] {
    UnsafeBufferPointer(start: rows, count: count).map { row in
        Row(id: string(row.id, row.id_len), status: string(row.status, row.status_len),
            notes: string(row.notes, row.notes_len), total: row.total_cents)
    }
}
final class Rows { var values: [Row] = [] }
let rowsVisitor: hoya_reactive_rows_callback = { context, rows, count in
    Unmanaged<Rows>.fromOpaque(context!).takeUnretainedValue().values = copyRows(rows, count)
}
func query(_ handle: UInt64, _ status: String = "pending") -> (Int32, [Row]) {
    let result = Rows()
    let code = status.withCString {
        hoya_reactive_query(handle, $0, rowsVisitor, Unmanaged.passUnretained(result).toOpaque())
    }
    return (code, result.values)
}
func insert(_ handle: UInt64, _ status: String = "pending", _ notes: String = "🌿", _ total: Int64 = 1200) -> Int32 {
    status.withCString { state in
        notes.withCString { hoya_reactive_insert(handle, state, $0, total) }
    }
}
func replace(_ handle: UInt64, _ id: String, _ status: String, _ notes: String, _ total: Int64) -> Int32 {
    id.withCString { key in status.withCString { state in
        notes.withCString { hoya_reactive_replace(handle, key, state, $0, total) }
    } }
}
func observe(_ handle: UInt64, _ mode: UInt8 = 0) -> UInt64 {
    hoya_reactive_observe(handle, "pending", mode)
}
func threadID() -> UInt64 {
    var value: UInt64 = 0
    pthread_threadid_np(nil, &value)
    return value
}
struct Event: Equatable {
    let token: UInt64
    let kind: UInt8
    let count: Int
    let rows: [Row]
    let error: UInt32
}
final class Events {
    let handle: UInt64
    var values: [Event] = []
    var threads: [UInt64] = []
    var times: [UInt64] = []
    var queried: [[Row]] = []
    var failures: [String] = []
    var cancel: UInt64? = nil
    var close = false
    var write = false
    var recursive = false
    var actionCodes: [Int32] = []
    init(_ handle: UInt64) { self.handle = handle }
}
let eventVisitor: hoya_reactive_event_callback = { context, token, kind, count, rows, rowCount, error in
    let result = Unmanaged<Events>.fromOpaque(context!).takeUnretainedValue()
    result.times.append(DispatchTime.now().uptimeNanoseconds)
    result.threads.append(threadID())
    result.values.append(Event(token: token, kind: kind, count: count,
        rows: copyRows(rows, rowCount), error: error))
    let current = query(result.handle)
    if current.0 != 0 { result.failures.append("callback query code \(current.0)") }
    result.queried.append(current.1)
    if result.values.count == 1 {
        if result.recursive {
            result.actionCodes.append(hoya_reactive_drain(result.handle, 1000, eventVisitor, context))
        }
        if result.write { result.actionCodes.append(insert(result.handle, "pending", "callback", 77)) }
        if let cancel = result.cancel { result.actionCodes.append(hoya_reactive_unsubscribe(result.handle, cancel)) }
        if result.close { result.actionCodes.append(hoya_reactive_close(result.handle)) }
    }
    // The borrowed snapshot must remain valid even if re-entry writes or closes
    // its original owner; the bridge owns it until this callback returns.
    if copyRows(rows, rowCount) != result.values.last!.rows {
        result.failures.append("borrowed snapshot changed during callback re-entry")
    }
}
func drain(_ result: Events, _ now: UInt64 = 0) -> Int32 {
    hoya_reactive_drain(result.handle, now, eventVisitor, Unmanaged.passUnretained(result).toOpaque())
}

@MainActor func snapshots() throws {
    let handle = hoya_reactive_open()
    try require(handle != 0, "R14A production database opens")
    defer { precondition(hoya_reactive_close(handle) == 0) }
    let standard = observe(handle), finer = observe(handle, 2), count = observe(handle, 1)
    try require(standard != 0 && finer != 0 && count != 0, "R14A registrations")
    let initial = Events(handle)
    try require(drain(initial) == 0 && initial.values.count == 3, "R14A initial events")
    for event in initial.values {
        try require(event.rows.isEmpty && event.count == 0 && event.error == 0, "R14A exact empty snapshot/count")
        try require(event.kind == (event.token == count ? 1 : 0), "R14A exact initial event kinds")
    }
    try require(hoya_reactive_drain(handle, 0, nil, nil) == 2, "R14A missing visitor rejected")
    try require(hoya_reactive_insert(handle, nil, "notes", 1) == 2, "R14A null input rejected")
    try require(hoya_reactive_observe(handle, "pending", 99) == 0, "R14A unknown mode rejected")
    try require(insert(handle, "pending", "注文🌿", Int64.min) == 0, "R14A typed write")
    let first = Events(handle)
    try require(drain(first, 1) == 0 && first.values.count == 2, "R14A rows immediate/count throttled")
    let original = Row(id: "local:1", status: "pending", notes: "注文🌿", total: Int64.min)
    try require(Set(first.values.map(\.token)) == Set([standard, finer]), "R14A row registrations")
    try require(first.values.allSatisfy { $0.kind == 0 && $0.rows == [original] && $0.error == 0 }, "R14A exact owned rows")
    let deferred = Events(handle)
    try require(drain(deferred, 15) == 0 && deferred.values.isEmpty, "R14A count waits deadline")
    try require(drain(deferred, 16) == 0 && deferred.values == [Event(token: count, kind: 1, count: 1, rows: [], error: 0)], "R14A exact count")
    try require(replace(handle, "local:1", "pending", "untracked", Int64.min) == 0, "R14A unrelated edit")
    let silent = Events(handle)
    try require(drain(silent, 17) == 0 && silent.values.isEmpty, "R14A untracked edit silent")
    try require(replace(handle, "local:1", "pending", "changed", Int64.max) == 0, "R14A tracked edit")
    let changed = Events(handle)
    try require(drain(changed, 18) == 0 && changed.values.count == 1 && changed.values[0].token == finer
        && changed.values[0].rows == [Row(id: "local:1", status: "pending", notes: "changed", total: Int64.max)], "R14A tracked snapshot")
    try require(first.values[0].rows == [original], "R14A copied old rows remain owned")
    try require(hoya_reactive_remove(handle, "local:1") == 0, "R14A remove")
    let removed = Events(handle)
    try require(drain(removed, 32) == 0 && removed.values.count == 3
        && removed.values.allSatisfy { $0.count == 0 && $0.rows.isEmpty }, "R14A leave membership/count")
    try require(query(handle).1.isEmpty, "R14A callback reads current state")
}

@MainActor func callbacks() throws {
    let handle = hoya_reactive_open()
    try require(handle != 0, "R14B production database opens")
    var closed = false
    defer { if !closed { precondition(hoya_reactive_close(handle) == 0) } }
    let token = observe(handle)
    try require(token != 0, "R14B observe")
    let first = Events(handle)
    first.recursive = true; first.write = true
    try require(drain(first) == 0 && first.values.count == 1 && first.actionCodes == [5, 0]
        && first.failures.isEmpty && first.queried == [[]], "R14B unlocked read/write and recursive guard")
    let next = Events(handle)
    try require(drain(next, 1) == 0 && next.values.count == 1
        && next.values[0].rows == [Row(id: "local:1", status: "pending", notes: "callback", total: 77)], "R14B callback write on next drain")
    let late = observe(handle)
    try require(late != 0 && insert(handle) == 0, "R14B queue events before cancel")
    let canceled = Events(handle); canceled.cancel = late
    try require(drain(canceled, 2) == 0 && canceled.actionCodes == [0]
        && !canceled.values.dropFirst().contains { $0.token == late }, "R14B cancel suppresses extracted events")
    try require(hoya_reactive_unsubscribe(handle, late) == 1, "R14B stale token rejected")
    try require(insert(handle) == 0 && insert(handle) == 0, "R14B queue events before close")
    let closing = Events(handle); closing.close = true
    let code = drain(closing, 3)
    closed = closing.actionCodes == [0]
    try require(code == 0 && closed && closing.values.count == 1 && closing.failures.isEmpty,
        "R14B close suppresses remaining callbacks and preserves borrowed snapshot")
    try require(query(handle).0 == 1 && drain(Events(handle), 4) == 1
        && hoya_reactive_close(handle) == 1, "R14B closed owner rejects calls")
}

@MainActor func timings() throws {
    let a = hoya_reactive_open()
    try require(a != 0, "R14C production database opens")
    let oldToken = observe(a)
    try require(oldToken != 0 && hoya_reactive_close(a) == 0, "R14C close old owner")
    let b = hoya_reactive_open()
    try require(b != 0 && b != a, "R14C owner handles never reused")
    defer { precondition(hoya_reactive_close(b) == 0) }
    let token = observe(b)
    try require(token != 0 && token != oldToken && hoya_reactive_unsubscribe(b, oldToken) == 1,
        "R14C old subscription cannot cancel new owner")
    try require(drain(Events(b)) == 0, "R14C initial")
    let caller = threadID()
    var samples: [UInt64] = []
    for index in 0..<12 {
        let before = DispatchTime.now().uptimeNanoseconds
        try require(insert(b, "pending", "timing-\(index)", Int64(index)) == 0, "R15 write")
        let result = Events(b)
        try require(drain(result, UInt64(index + 1)) == 0 && result.values.count == 1
            && result.values[0].count == index + 1 && result.values[0].rows.count == index + 1
            && result.threads == [caller] && result.failures.isEmpty, "R15 exact delivery and caller affinity")
        samples.append(result.times[0] - before)
    }
    print("R15 native insertion-to-callback samples ns=\(samples), drainCallerThread=\(caller), cadence=explicit immediate drain; measured to callback entry")
    let final = Events(b)
    try require(hoya_reactive_unsubscribe(b, token) == 0 && insert(b) == 0
        && drain(final, 20) == 0 && final.values.isEmpty, "R15 final unsubscribe")
}

@MainActor func projectedFailure() throws {
    let handle = hoya_reactive_open()
    try require(handle != 0, "Q05 open")
    var closed = false
    defer { if !closed { precondition(hoya_reactive_close(handle) == 0) } }
    let rowToken = observe(handle), countToken = observe(handle, 1)
    try require(rowToken != 0 && countToken != 0, "Q05 observations")
    let initial = Events(handle)
    try require(drain(initial, 100) == 0 && initial.values.count == 2
        && initial.values.allSatisfy { $0.count == 0 && $0.error == 0 }, "Q05 initial delivery")
    let caller = threadID()
    let committed = Row(id: "local:1", status: "pending", notes: "fault🌿", total: Int64.max)
    try require(hoya_reactive_fail_next_project(handle) == 0, "Q05 arm one projected failure")
    try require(insert(handle, "pending", "fault🌿", Int64.max) == 0,
        "Q05 post-commit read failure must not relabel write")
    let failed = Events(handle)
    try require(drain(failed, 101) == 0 && failed.values.count == 2
        && Set(failed.values.map(\.token)) == Set([rowToken, countToken])
        && failed.values.allSatisfy { $0.kind == 2 && $0.error == 7 && $0.rows.isEmpty && $0.count == 0 },
        "Q05 typed projected errors reach actual Swift callbacks")
    try require(failed.failures.isEmpty && failed.queried == [[committed], [committed]]
        && failed.threads == [caller, caller], "Q05 error callbacks reenter unlocked committed reads on caller thread")
    for instant in [UInt64(102), UInt64(103)] {
        try require(hoya_reactive_fail_next_project(handle) == 0 && hoya_reactive_refresh(handle) == 0,
            "Q05 retry command acknowledged")
        let retried = Events(handle)
        try require(drain(retried, instant) == 0 && retried.values.count == 2
            && retried.values.allSatisfy { $0.kind == 2 && $0.error == 7 && $0.rows.isEmpty },
            "Q05 every failed explicit refresh delivers typed errors")
        try require(retried.failures.isEmpty && retried.queried == [[committed], [committed]],
            "Q05 repeated errors preserve committed ownership")
    }
    try require(hoya_reactive_refresh(handle) == 0, "Q05 successful explicit retry")
    let recovered = Events(handle)
    try require(drain(recovered, 104) == 0 && recovered.values == [
        Event(token: rowToken, kind: 0, count: 1, rows: [committed], error: 0)
    ], "Q05 exact owned row recovery while count retains deadline")
    let waiting = Events(handle)
    try require(drain(waiting, 115) == 0 && waiting.values.isEmpty,
        "Q05 errors cannot reset the count deadline")
    let count = Events(handle)
    try require(drain(count, 116) == 0 && count.values == [
        Event(token: countToken, kind: 1, count: 1, rows: [], error: 0)
    ], "Q05 exact count recovers at original sixteen-millisecond deadline")
    try require(failed.values.allSatisfy { $0.rows.isEmpty && $0.error == 7 }
        && recovered.values[0].rows == [committed], "Q05 earlier copied error and row values stay owned")
    try require(hoya_reactive_fail_next_project(handle) == 0 && insert(handle, "pending", "second", 2) == 0,
        "Q05 queue another failure")
    try require(hoya_reactive_unsubscribe(handle, rowToken) == 0, "Q05 cancel queued row error")
    let closing = Events(handle); closing.recursive = true; closing.close = true
    let closeDrain = drain(closing, 117)
    closed = closing.actionCodes == [5, 0]
    try require(closeDrain == 0 && closed && closing.values == [
        Event(token: countToken, kind: 2, count: 0, rows: [], error: 7)
    ] && closing.failures.isEmpty, "Q05 error callback closes owner and canceled error stays suppressed")
    try require(hoya_reactive_fail_next_project(handle) == 1 && hoya_reactive_refresh(handle) == 1
        && drain(Events(handle), 118) == 1, "Q05 stale fixture controls and closed delivery reject")
}

@main struct Main {
    @MainActor static func main() {
        var failures = 0
        let cases: [(String, () throws -> Void)] = [
            ("R14A owned rows/count/columns", snapshots),
            ("R14B reentrant callback lifecycle", callbacks),
            ("R14C/R15 stale owners and timing", timings),
            ("Q05 projected error delivery/recovery", projectedFailure),
        ]
        for (name, test) in cases {
            do { try test(); print("PASS \(name)") }
            catch { failures += 1; print("FAIL \(name): \(error)") }
        }
        print("Swift Phase3 runtime: \(ProcessInfo.processInfo.operatingSystemVersionString)")
        exit(failures == 0 ? 0 : 1)
    }
}
