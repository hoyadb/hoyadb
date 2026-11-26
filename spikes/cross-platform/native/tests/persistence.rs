//! SPEC-0021 / TC-0021 P01–P06; bounded synthetic SQLite fixtures only.
use hoyadb_native_spike::*;
use rusqlite::Connection;
use std::ffi::{CString, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-persist-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.join("fixture.sqlite")
    }

    fn open(&self) -> u64 {
        let path = CString::new(self.path().to_str().unwrap()).unwrap();
        unsafe { hoya_open_persistent(path.as_ptr()) }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn insert(handle: u64, id: &str) -> i32 {
    let id = CString::new(id).unwrap();
    unsafe { hoya_insert(handle, id.as_ptr(), c"pending".as_ptr(), i64::MIN, 1) }
}

unsafe extern "C" fn row(
    ctx: *mut c_void,
    id: *const u8,
    len: usize,
    _status: *const u8,
    _status_len: usize,
    _total: i64,
    _paid: u8,
) {
    let rows = unsafe { &mut *ctx.cast::<Vec<String>>() };
    rows.push(String::from_utf8(unsafe { std::slice::from_raw_parts(id, len) }.to_vec()).unwrap());
}

fn rows(handle: u64, scope: Option<u64>) -> Vec<String> {
    let mut rows = Vec::<String>::new();
    let ctx = std::ptr::from_mut(&mut rows).cast();
    let code = unsafe {
        match scope {
            Some(scope) => hoya_scope_query(handle, scope, c"pending".as_ptr(), Some(row), ctx),
            None => hoya_query(handle, c"pending".as_ptr(), Some(row), ctx),
        }
    };
    assert_eq!(code, 0);
    rows.sort();
    rows
}

unsafe extern "C" fn event(ctx: *mut c_void, _token: u64, count: usize) {
    unsafe { &mut *ctx.cast::<Vec<usize>>() }.push(count);
}

fn events(handle: u64) -> Vec<usize> {
    let mut events = Vec::<usize>::new();
    assert_eq!(
        unsafe { hoya_drain(handle, Some(event), std::ptr::from_mut(&mut events).cast()) },
        0
    );
    events
}

#[test]
fn p01_p02_p04_reopen_restores_committed_rows_only() {
    let fixture = Fixture::new();
    let handle = fixture.open();
    assert_ne!(handle, 0);
    assert_eq!(insert(handle, "🌿"), 0);
    let sub = unsafe { hoya_observe(handle, c"pending".as_ptr()) };
    assert_ne!(sub, 0);
    assert_eq!(events(handle), vec![1]);
    let scope = hoya_begin_scope(handle);
    for id in [c"a", c"b"] {
        assert_eq!(
            unsafe {
                hoya_scope_insert(handle, scope, id.as_ptr(), c"pending".as_ptr(), i64::MAX, 0)
            },
            0
        );
    }
    assert_eq!(hoya_finish_scope(handle, scope, 1), 0);
    assert_eq!(events(handle), vec![3]);
    let aborted = hoya_begin_scope(handle);
    assert_eq!(
        unsafe {
            hoya_scope_insert(
                handle,
                aborted,
                c"aborted".as_ptr(),
                c"pending".as_ptr(),
                1,
                1,
            )
        },
        0
    );
    assert_eq!(hoya_finish_scope(handle, aborted, 0), 0);
    assert_eq!(hoya_close(handle), 0);
    let restored = fixture.open();
    assert_ne!(restored, 0);
    assert_eq!(rows(restored, None), vec!["a", "b", "🌿"]);
    assert!(events(restored).is_empty());
    assert_ne!(hoya_begin_scope(restored), 0);
    assert_eq!(hoya_close(restored), 0);
    println!("SQLite {}", rusqlite::version());
}

#[test]
fn p03_sqlite_failure_preserves_memory_disk_events_and_scope() {
    let fixture = Fixture::new();
    let handle = fixture.open();
    assert_ne!(handle, 0);
    assert_eq!(insert(handle, "seed"), 0);
    assert_ne!(unsafe { hoya_observe(handle, c"pending".as_ptr()) }, 0);
    let conn = Connection::open(fixture.path()).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_fail BEFORE INSERT ON hoya_spike_rows WHEN NEW.id='fail' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
    assert_eq!(insert(handle, "fail"), 4);
    assert_eq!(rows(handle, None), vec!["seed"]);
    assert_eq!(events(handle), vec![1]);
    let scope = hoya_begin_scope(handle);
    for id in [c"candidate", c"fail"] {
        assert_eq!(
            unsafe { hoya_scope_insert(handle, scope, id.as_ptr(), c"pending".as_ptr(), 3, 1) },
            0
        );
    }
    assert_ne!(unsafe { hoya_observe(handle, c"pending".as_ptr()) }, 0);
    assert_eq!(hoya_finish_scope(handle, scope, 1), 4);
    assert_eq!(rows(handle, None), vec!["seed"]);
    assert_eq!(rows(handle, Some(scope)), vec!["candidate", "fail", "seed"]);
    assert_eq!(events(handle), vec![1]);
    let disk: Vec<String> = conn
        .prepare("SELECT id FROM hoya_spike_rows ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(disk, vec!["seed"]);
    conn.execute_batch("DROP TRIGGER reject_fail").unwrap();
    assert_eq!(hoya_finish_scope(handle, scope, 1), 0);
    assert_eq!(events(handle), vec![3, 3]);
    assert_eq!(hoya_close(handle), 0);
    let reopened = fixture.open();
    assert_eq!(rows(reopened, None), vec!["candidate", "fail", "seed"]);
    assert_eq!(hoya_close(reopened), 0);
}

#[test]
fn p05_invalid_paths_and_incompatible_data_are_rejected() {
    assert_eq!(unsafe { hoya_open_persistent(std::ptr::null()) }, 0);
    assert_eq!(unsafe { hoya_open_persistent(c"".as_ptr()) }, 0);
    assert_eq!(unsafe { hoya_open_persistent(c":memory:".as_ptr()) }, 0);
    let uri_handles: Vec<_> = [
        c"file::memory:",
        c"file:hoya-memory?mode=memory&cache=shared",
    ]
    .into_iter()
    .map(|uri| unsafe { hoya_open_persistent(uri.as_ptr()) })
    .collect();
    assert_eq!(
        uri_handles,
        vec![0, 0],
        "SQLite URI paths must not create memory-backed handles"
    );
    let bytes = [255_u8, 0];
    assert_eq!(unsafe { hoya_open_persistent(bytes.as_ptr().cast()) }, 0);
    let fixture = Fixture::new();
    let directory = CString::new(fixture.0.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { hoya_open_persistent(directory.as_ptr()) }, 0);
    let original = b"not a sqlite database";
    std::fs::write(fixture.path(), original).unwrap();
    assert_eq!(fixture.open(), 0);
    assert_eq!(std::fs::read(fixture.path()).unwrap(), original);
}

#[test]
fn p05_malformed_boolean_types_and_duplicate_ids_are_rejected() {
    for values in [
        "('a','pending',1,2)",
        "('a','pending','bad',1)",
        "(1,'pending',1,1)",
        "('a',1,1,1)",
        "('a','pending',1.5,1)",
        "('a','pending',1,'true')",
        "(NULL,'pending',1,1)",
        "('a','pending',1,1),('a','pending',2,0)",
    ] {
        let fixture = Fixture::new();
        let conn = Connection::open(fixture.path()).unwrap();
        conn.execute_batch("CREATE TABLE hoya_spike_rows(id,status,total_cents,paid)")
            .unwrap();
        conn.execute_batch(&format!("INSERT INTO hoya_spike_rows VALUES {values}"))
            .unwrap();
        assert_eq!(fixture.open(), 0);
    }
}

#[test]
fn p06_memory_and_persistent_stores_remain_independent() {
    let fixture = Fixture::new();
    let disk = fixture.open();
    assert_ne!(disk, 0);
    let memory = hoya_open();
    assert_eq!(insert(disk, "disk"), 0);
    assert_eq!(insert(memory, "memory"), 0);
    assert_eq!(rows(disk, None), vec!["disk"]);
    assert_eq!(rows(memory, None), vec!["memory"]);
    assert_eq!(hoya_close(disk), 0);
    assert_eq!(hoya_close(memory), 0);
    let restored = fixture.open();
    assert_eq!(rows(restored, None), vec!["disk"]);
    assert_eq!(hoya_close(restored), 0);
}
