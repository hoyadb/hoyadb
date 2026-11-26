// TC-0018 / SPEC-0018: actual Swift -> Rust -> Swift integration cases.
import Darwin

struct Row: Equatable {
    let id: String
    let status: String
    let total: Int64
    let paid: Bool
}

enum TestFailure: Error { case failed(String) }

func expect(_ condition: Bool, _ message: String) throws {
    if !condition { throw TestFailure.failed(message) }
}

final class Rows {
    var values: [Row] = []
    var nestedHandle: UInt64? = nil
    var nested: [Row] = []
    var nestedStatus: Int32 = -1
}

let visitor: hoya_row_callback = { context, id, idLength, status, statusLength, total, paid in
    let rows = Unmanaged<Rows>.fromOpaque(context!).takeUnretainedValue()
    rows.values.append(Row(
        id: String(decoding: UnsafeBufferPointer(start: id, count: idLength), as: UTF8.self),
        status: String(decoding: UnsafeBufferPointer(start: status, count: statusLength), as: UTF8.self),
        total: total, paid: paid == 1))
    if let handle = rows.nestedHandle {
        let result = query(handle, "pending")
        rows.nestedStatus = result.0
        rows.nested = result.1
    }
}

func query(_ handle: UInt64, _ status: String, _ rows: Rows = Rows()) -> (Int32, [Row]) {
    let result = status.withCString {
        hoya_query(handle, $0, visitor, Unmanaged.passUnretained(rows).toOpaque())
    }
    return (result, rows.values.sorted { $0.id < $1.id })
}

func open() throws -> UInt64 {
    let handle = hoya_open()
    try expect(handle != 0, "open must return a live handle")
    return handle
}

func insert(_ handle: UInt64, _ row: Row) throws {
    let result = row.id.withCString { id in
        row.status.withCString { status in
            hoya_insert(handle, id, status, row.total, row.paid ? 1 : 0)
        }
    }
    try expect(result == 0, "insert must succeed, got \(result)")
}

func n01() throws {
    let handle = try open()
    defer { precondition(hoya_close(handle) == 0) }
    let a = Row(id: "a", status: "pending", total: 1200, paid: false)
    let b = Row(id: "b", status: "pending", total: 3500, paid: true)
    try insert(handle, b)
    try insert(handle, Row(id: "c", status: "complete", total: 900, paid: true))
    try insert(handle, a)
    let result = query(handle, "pending")
    try expect(result.0 == 0 && result.1 == [a, b], "filter must preserve complete matching rows")
    print("rows: \(result.1)")
}

func n02() throws {
    let empty = try open(), complete = try open(), other = try open()
    defer { for handle in [empty, complete, other] { precondition(hoya_close(handle) == 0) } }
    try insert(complete, Row(id: "a", status: "complete", total: 1, paid: false))
    try insert(other, Row(id: "b", status: "pending", total: 2, paid: true))
    for handle in [empty, complete] {
        let result = query(handle, "pending")
        try expect(result.0 == 0 && result.1.isEmpty, "stores must stay independent")
    }
}

func n03() throws {
    let handle = try open()
    defer { precondition(hoya_close(handle) == 0) }
    let expected = [Row(id: "a-🌿", status: "保留", total: Int64.min, paid: false),
                    Row(id: "b-é", status: "保留", total: Int64.max, paid: true)]
    for row in expected {
        // Overwrite the source bytes after return to prove Rust owns its copy.
        var id = Array(row.id.utf8CString), status = Array(row.status.utf8CString)
        let result = id.withUnsafeBufferPointer { ids in
            status.withUnsafeBufferPointer { statuses in
                hoya_insert(handle, ids.baseAddress, statuses.baseAddress, row.total, row.paid ? 1 : 0)
            }
        }
        try expect(result == 0, "Unicode insert must succeed")
        for index in id.indices { id[index] = 0 }
        for index in status.indices { status[index] = 0 }
    }
    let result = query(handle, "保留")
    try expect(result.0 == 0 && result.1 == expected, "owned strings and i64 extrema must roundtrip")
}

func n04() throws {
    let handle = try open()
    defer { precondition(hoya_close(handle) == 0) }
    let disposed = try open()
    try expect(hoya_close(disposed) == 0, "first disposal succeeds")
    try expect(hoya_close(disposed) == 1, "second disposal rejects stale handle")
    let next = try open()
    defer { precondition(hoya_close(next) == 0) }
    try expect(next != disposed && next != handle, "handles must not be reused")
    for invalid in [UInt64(0), UInt64.max, disposed] {
        try expect(hoya_close(invalid) == 1, "invalid close must reject")
        try expect(hoya_insert(invalid, "a", "pending", 1, 0) == 1, "invalid insert must reject")
        try expect(query(invalid, "pending").0 == 1, "invalid query must reject")
    }
    try expect(hoya_insert(handle, nil, "pending", 1, 0) == 2, "null ID must reject")
    try expect(hoya_insert(handle, "a", nil, 1, 0) == 2, "null status must reject")
    try expect(hoya_insert(handle, "a", "pending", 1, 2) == 2, "paid 2 must reject")
    let bytes: [CChar] = [-1, 0]
    try bytes.withUnsafeBufferPointer { invalid in
        try expect(hoya_insert(handle, invalid.baseAddress, "pending", 1, 0) == 2, "invalid ID UTF-8 must reject")
        try expect(hoya_insert(handle, "a", invalid.baseAddress, 1, 0) == 2, "invalid status UTF-8 must reject")
        try expect(hoya_query(handle, invalid.baseAddress, visitor, nil) == 2, "invalid query UTF-8 must reject")
    }
    try expect(hoya_query(handle, nil, visitor, nil) == 2, "null query must reject")
    try expect(hoya_query(handle, "pending", nil, nil) == 2, "null visitor must reject")
    let result = query(handle, "pending")
    try expect(result.0 == 0 && result.1.isEmpty, "rejected operations must not insert rows")
}

func n05() throws {
    let handle = try open()
    defer { precondition(hoya_close(handle) == 0) }
    let expected = Row(id: "a", status: "pending", total: 1, paid: true)
    try insert(handle, expected)
    let rows = Rows()
    rows.nestedHandle = handle
    let result = query(handle, "pending", rows)
    try expect(result.0 == 0 && result.1 == [expected], "outer query must succeed")
    try expect(rows.nestedStatus == 0 && rows.nested == [expected], "nested query must succeed")
}

var failures = 0
for (name, test) in [("N01", n01), ("N02", n02), ("N03", n03), ("N04", n04), ("N05", n05)] {
    do { try test(); print("PASS \(name)") }
    catch { failures += 1; print("FAIL \(name): \(error)") }
}
print("native: \(5 - failures) passed, \(failures) failed")
exit(failures == 0 ? 0 : 1)
