// SPEC-0018 / TC-0018 N07–N09. Compile separately with -parse-as-library.
import Darwin
import Dispatch

enum Failure: Error { case check(String), handler }
func require(_ value: Bool, _ message: String) throws {
    if !value { throw Failure.check(message) }
}
struct Row: Equatable {
    let id: String
    let status: String
    let total: Int64
    let paid: Bool
}
final class Rows { var values: [Row] = [] }
let rowVisitor: hoya_row_callback = { context, id, idCount, status, statusCount, total, paid in
    let rows = Unmanaged<Rows>.fromOpaque(context!).takeUnretainedValue()
    rows.values.append(Row(
        id: String(decoding: UnsafeBufferPointer(start: id, count: idCount), as: UTF8.self),
        status: String(decoding: UnsafeBufferPointer(start: status, count: statusCount), as: UTF8.self),
        total: total, paid: paid == 1))
}
func query(_ handle: UInt64, scope: UInt64? = nil) -> (Int32, [Row]) {
    let rows = Rows()
    let context = Unmanaged.passUnretained(rows).toOpaque()
    let code = "pending".withCString { status in
        if let scope { return hoya_scope_query(handle, scope, status, rowVisitor, context) }
        return hoya_query(handle, status, rowVisitor, context)
    }
    return (code, rows.values.sorted { $0.id < $1.id })
}
func insert(_ handle: UInt64, _ id: String, scope: UInt64? = nil,
            status: String = "pending") -> Int32 {
    id.withCString { key in
        status.withCString { value in
            if let scope { return hoya_scope_insert(handle, scope, key, value, 1200, 1) }
            return hoya_insert(handle, key, value, 1200, 1)
        }
    }
}
func threadID() -> UInt64 {
    var value: UInt64 = 0
    pthread_threadid_np(nil, &value)
    return value
}
final class Events {
    let handle: UInt64
    var values: [(UInt64, Int)] = []
    var queried: [[Row]] = []
    var queryCodes: [Int32] = []
    var threads: [UInt64] = []
    var times: [UInt64] = []
    var unsubscribeOnFirst: UInt64? = nil
    var unsubscribeCode: Int32? = nil
    var closeOnFirst = false
    var closeCode: Int32? = nil
    var insertOnFirst: String? = nil
    var insertCode: Int32? = nil
    init(_ handle: UInt64) { self.handle = handle }
}
let eventVisitor: hoya_event_callback = { context, token, count in
    let events = Unmanaged<Events>.fromOpaque(context!).takeUnretainedValue()
    events.values.append((token, count))
    events.threads.append(threadID())
    events.times.append(DispatchTime.now().uptimeNanoseconds)
    let result = query(events.handle)
    events.queryCodes.append(result.0)
    events.queried.append(result.1)
    if events.values.count == 1, let token = events.unsubscribeOnFirst {
        events.unsubscribeCode = hoya_unobserve(events.handle, token)
    }
    if events.values.count == 1, let id = events.insertOnFirst {
        events.insertCode = insert(events.handle, id)
    }
    if events.values.count == 1 && events.closeOnFirst {
        events.closeCode = hoya_close(events.handle)
    }
}
func drain(_ events: Events) -> Int32 {
    hoya_drain(events.handle, eventVisitor, Unmanaged.passUnretained(events).toOpaque())
}

actor Gate {
    private var entered = false
    private var ready: CheckedContinuation<Void, Never>?
    private var release: CheckedContinuation<Void, Never>?
    func block() async {
        await withCheckedContinuation { continuation in
            release = continuation
            entered = true
            ready?.resume()
            ready = nil
        }
    }
    func waitUntilEntered() async {
        if entered { return }
        await withCheckedContinuation { ready = $0 }
    }
    func unblock() { release?.resume(); release = nil }
}

@MainActor func n07() throws {
    let handle = hoya_open()
    try require(handle != 0, "N07 open")
    var closed = false
    defer { if !closed { precondition(hoya_close(handle) == 0) } }
    let token = hoya_observe(handle, "pending")
    try require(token != 0, "N07 observe")
    try require(hoya_drain(handle, nil, nil) == 2, "N07 null drain preserves pending event")
    let initial = Events(handle)
    try require(drain(initial) == 0, "N07 initial drain")
    try require(initial.values.count == 1 && initial.values[0].0 == token
                && initial.values[0].1 == 0, "N07 initial event")
    let start = DispatchTime.now().uptimeNanoseconds, callingThread = threadID()
    try require(insert(handle, "a") == 0, "N07 matching insert")
    let changed = Events(handle)
    try require(drain(changed) == 0 && changed.values.count == 1, "N07 changed event")
    try require(changed.values[0].0 == token && changed.values[0].1 == 1
                && changed.queryCodes == [0] && changed.queried[0].map(\.id) == ["a"],
                "N07 reentrant complete result")
    try require(changed.threads == [callingThread], "N07 synchronous drain thread")
    print("N07 insertion-to-callback ns=\(changed.times[0] - start), thread=\(callingThread); includes explicit immediate-drain scheduling")
    try require(insert(handle, "irrelevant", status: "complete") == 0, "N07 irrelevant insert")
    let irrelevant = Events(handle)
    try require(drain(irrelevant) == 0 && irrelevant.values.isEmpty, "N07 no irrelevant emission")
    try require(insert(handle, "b") == 0 && hoya_unobserve(handle, token) == 0,
                "N07 remove pending event")
    try require(hoya_unobserve(handle, token) == 1, "N07 stale token")
    try require(insert(handle, "c") == 0, "N07 insertion after unsubscribe")
    let removed = Events(handle)
    try require(drain(removed) == 0 && removed.values.isEmpty, "N07 no events after unsubscribe")
    let reentrantToken = hoya_observe(handle, "pending")
    try require(reentrantToken != 0 && insert(handle, "d") == 0, "N07 queue two events")
    let reentrant = Events(handle)
    reentrant.unsubscribeOnFirst = reentrantToken
    try require(drain(reentrant) == 0 && reentrant.values.count == 1
                && reentrant.unsubscribeCode == 0, "N07 unsubscribe suppresses extracted event")
    try require(hoya_observe(handle, "pending") != 0, "N07 registration before close")
    try require(hoya_close(handle) == 0, "N07 close")
    closed = true
    try require(drain(Events(handle)) == 1, "N07 disposed drain")

    let closing = hoya_open()
    try require(closing != 0, "N07 close-callback store")
    var callbackClosed = false
    defer { if !callbackClosed { precondition(hoya_close(closing) == 0) } }
    try require(hoya_observe(closing, "pending") != 0 && insert(closing, "a") == 0,
                "N07 queue events before callback close")
    let closeEvents = Events(closing)
    closeEvents.closeOnFirst = true
    let closeDrain = drain(closeEvents)
    callbackClosed = closeEvents.closeCode == 0
    try require(closeDrain == 0 && callbackClosed && closeEvents.values.count == 1,
                "N07 callback close suppresses remaining extracted event")
    try require(query(closing).0 == 1 && hoya_begin_scope(closing) == 0,
                "N07 callback-disposed store rejects later calls")

    let writing = hoya_open()
    try require(writing != 0, "N07 callback-write store")
    defer { precondition(hoya_close(writing) == 0) }
    let writingToken = hoya_observe(writing, "pending")
    try require(writingToken != 0, "N07 callback-write observe")
    let firstDrain = Events(writing)
    firstDrain.insertOnFirst = "callback-row"
    try require(drain(firstDrain) == 0 && firstDrain.values.count == 1
                && firstDrain.insertCode == 0, "N07 callback insertion does not extend current drain")
    let nextDrain = Events(writing)
    try require(drain(nextDrain) == 0 && nextDrain.values.count == 1
                && nextDrain.values[0].0 == writingToken && nextDrain.values[0].1 == 1
                && nextDrain.queried[0].map(\.id) == ["callback-row"],
                "N07 callback insertion appears on next drain")
    let finalDrain = Events(writing)
    try require(drain(finalDrain) == 0 && finalDrain.values.isEmpty, "N07 final drain empty")
}

@MainActor func n08() async throws {
    let handle = hoya_open()
    try require(handle != 0, "N08 open")
    defer { precondition(hoya_close(handle) == 0) }
    try require(hoya_observe(handle, "pending") != 0, "N08 observe")
    try require(drain(Events(handle)) == 0, "N08 initial drain")
    let scope = hoya_begin_scope(handle)
    try require(scope != 0, "N08 begin")
    let gate = Gate()
    let handler = Task { @MainActor in
        let staged = insert(handle, "a", scope: scope) == 0
            && insert(handle, "b", scope: scope) == 0
        await gate.block()
        try require(staged, "N08 staged insert")
        let result = query(handle, scope: scope)
        try require(result.0 == 0 && result.1.map(\.id) == ["a", "b"], "N08 resumed scoped read")
        try require(hoya_finish_scope(handle, scope, 1) == 0, "N08 finish")
    }
    await gate.waitUntilEntered()
    let invalidCommit = hoya_finish_scope(handle, scope, 2)
    let stillBusy = hoya_begin_scope(handle) == 0
    let invalidNull = hoya_scope_insert(handle, scope, nil, "pending", 1, 0)
    let invalidBoolean = hoya_scope_insert(handle, scope, "invalid", "pending", 1, 2)
    let invalidBytes: [CChar] = [-1, 0]
    let invalidUTF8 = invalidBytes.withUnsafeBufferPointer {
        hoya_scope_insert(handle, scope, $0.baseAddress, "pending", 1, 0)
    }
    let inside = query(handle, scope: scope), outside = query(handle)
    let hidden = Events(handle)
    let hiddenCode = drain(hidden)
    await gate.unblock()
    try await handler.value
    try require(invalidCommit == 2 && stillBusy, "N08 invalid finish preserves active scope")
    try require(invalidNull == 2 && invalidBoolean == 2 && invalidUTF8 == 2,
                "N08 invalid scoped input rejects without staging")
    try require(inside.0 == 0 && inside.1.map(\.id) == ["a", "b"], "N08 staged visibility")
    try require(outside.0 == 0 && outside.1.isEmpty && hiddenCode == 0
                && hidden.values.isEmpty, "N08 outside isolation while suspended")
    let committed = query(handle), events = Events(handle)
    try require(committed.0 == 0 && committed.1.map(\.id) == ["a", "b"], "N08 atomic publication")
    try require(drain(events) == 0 && events.values.count == 1 && events.values[0].1 == 2
                && events.queried[0] == committed.1, "N08 one complete commit event")
}

@MainActor func n09() async throws {
    let handle = hoya_open()
    try require(handle != 0, "N09 open")
    defer { precondition(hoya_close(handle) == 0) }
    try require(hoya_observe(handle, "pending") != 0 && drain(Events(handle)) == 0, "N09 observe")
    let scope = hoya_begin_scope(handle)
    try require(scope != 0, "N09 begin")
    let gate = Gate()
    let handler = Task { @MainActor in
        let staged = insert(handle, "aborted", scope: scope)
        await gate.block()
        do {
            try require(staged == 0, "N09 stage")
            throw Failure.handler
        } catch {
            try require(hoya_finish_scope(handle, scope, 0) == 0, "N09 abort after throw")
            throw error
        }
    }
    await gate.waitUntilEntered()
    let busy = hoya_begin_scope(handle)
    await gate.unblock()
    do { try await handler.value; throw Failure.check("N09 handler must throw") }
    catch Failure.handler { }
    try require(busy == 0, "N09 second handler blocked")
    let outside = query(handle), events = Events(handle)
    try require(outside.0 == 0 && outside.1.isEmpty && drain(events) == 0
                && events.values.isEmpty, "N09 abort publishes nothing")
    try require(query(handle, scope: scope).0 == 1 && insert(handle, "stale", scope: scope) == 1
                && hoya_finish_scope(handle, scope, 1) == 1
                && hoya_finish_scope(handle, scope, 0) == 1, "N09 stale scope rejects")
    let fresh = hoya_begin_scope(handle)
    try require(fresh != 0 && fresh != scope && hoya_finish_scope(handle, fresh, 1) == 0,
                "N09 fresh finish")
    let next = hoya_begin_scope(handle)
    try require(next != 0 && next != fresh && hoya_finish_scope(handle, next, 0) == 0,
                "N09 slot released after finish")
}

@main struct Experiments {
    @MainActor static func main() async {
        var failures = 0
        do { try n07(); print("PASS N07") } catch { failures += 1; print("FAIL N07: \(error)") }
        do { try await n08(); print("PASS N08") } catch { failures += 1; print("FAIL N08: \(error)") }
        do { try await n09(); print("PASS N09") } catch { failures += 1; print("FAIL N09: \(error)") }
        print("native experiments: \(3 - failures) passed, \(failures) failed")
        exit(failures == 0 ? 0 : 1)
    }
}
