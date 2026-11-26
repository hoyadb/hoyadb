// TC-0021: invoke writer then reader as independent OS processes.
import Darwin

struct SavedRow: Equatable {
    let id: String
    let status: String
    let total: Int64
    let paid: Bool
}
final class SavedRows { var values: [SavedRow] = [] }
let visitor: hoya_row_callback = { context, id, idCount, status, statusCount, total, paid in
    let rows = Unmanaged<SavedRows>.fromOpaque(context!).takeUnretainedValue()
    rows.values.append(SavedRow(
        id: String(decoding: UnsafeBufferPointer(start: id, count: idCount), as: UTF8.self),
        status: String(decoding: UnsafeBufferPointer(start: status, count: statusCount), as: UTF8.self),
        total: total, paid: paid == 1))
}
func check(_ value: Bool, _ message: String) {
    if !value { print("FAIL persistence: \(message)"); exit(1) }
}
let expected = [
    SavedRow(id: "a-🌿", status: "pending", total: Int64.min, paid: false),
    SavedRow(id: "b-é", status: "complete", total: Int64.max, paid: true),
    SavedRow(id: "c", status: "pending", total: 1200, paid: true),
    SavedRow(id: "d", status: "pending", total: 3500, paid: false),
]
func insert(_ handle: UInt64, _ value: SavedRow, scope: UInt64? = nil) {
    let result = value.id.withCString { id in
        value.status.withCString { status in
            if let scope { return hoya_scope_insert(handle, scope, id, status, value.total, value.paid ? 1 : 0) }
            return hoya_insert(handle, id, status, value.total, value.paid ? 1 : 0)
        }
    }
    check(result == 0, "insert \(value.id), code \(result)")
}
check(CommandLine.arguments.count == 3, "usage: persistence writer|reader temporary-path")
let mode = CommandLine.arguments[1]
check(mode == "writer" || mode == "reader", "unknown mode")
let handle = CommandLine.arguments[2].withCString { hoya_open_persistent($0) }
check(handle != 0, "persistent open must return a live handle")
if mode == "writer" {
    for value in expected.prefix(2) { insert(handle, value) }
    let scope = hoya_begin_scope(handle)
    check(scope != 0, "begin committed scope")
    for value in expected.suffix(2) { insert(handle, value, scope: scope) }
    check(hoya_finish_scope(handle, scope, 1) == 0, "persist scope")
    let aborted = hoya_begin_scope(handle)
    check(aborted != 0, "begin aborted scope")
    insert(handle, SavedRow(id: "aborted", status: "pending", total: 99, paid: true), scope: aborted)
    check(hoya_finish_scope(handle, aborted, 0) == 0, "abort scope")
} else {
    for status in ["pending", "complete"] {
        let result = SavedRows()
        let code = status.withCString {
            hoya_query(handle, $0, visitor, Unmanaged.passUnretained(result).toOpaque())
        }
        check(code == 0, "restored query")
        let sorted = result.values.sorted { $0.id < $1.id }
        check(sorted == expected.filter { $0.status == status }, "exact restored rows, \(status)")
        print("restored rows: \(sorted)")
    }
    let scope = hoya_begin_scope(handle)
    check(scope != 0 && hoya_finish_scope(handle, scope, 0) == 0, "restore has no active scope")
}
check(hoya_close(handle) == 0, "close")
print("PASS persistence \(mode), pid=\(getpid()) P01/P02/P04")
