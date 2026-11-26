#[path = "../../../tests/conformance/command_recovery.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

// TC-0042 on the memory adapter: one test per case, run with --test-threads=1 because owners
// share the process-global coordinator. Restart here reopens the same storage object.
#[test]
fn tc0042_r1_ctrl() {
    cases::r1_ctrl(MemoryStorage::default);
}

#[test]
fn tc0042_m1() {
    cases::m1(MemoryStorage::default());
}

#[test]
fn tc0042_m2() {
    cases::m2(MemoryStorage::default());
}

#[test]
fn tc0042_m3() {
    cases::m3(MemoryStorage::default());
}

#[test]
fn tc0042_m4() {
    cases::m4(MemoryStorage::default());
}

#[test]
fn tc0042_rd1() {
    cases::rd1(MemoryStorage::default);
}

#[test]
fn tc0042_rd2() {
    cases::rd2(MemoryStorage::default());
}

#[test]
fn tc0042_rd3() {
    cases::rd3(MemoryStorage::default());
}

#[test]
fn tc0042_x1() {
    cases::x1(MemoryStorage::default());
}

#[test]
fn tc0042_x2() {
    cases::x2(MemoryStorage::default());
}

#[test]
fn tc0042_ctrl_k() {
    cases::ctrl_k(MemoryStorage::default());
}

#[test]
fn tc0042_et1() {
    cases::et1(MemoryStorage::default);
}

#[test]
fn tc0042_et2() {
    cases::et2(MemoryStorage::default());
}

#[test]
fn tc0042_et3() {
    cases::et3(MemoryStorage::default());
}

#[test]
fn tc0042_et4() {
    cases::et4(MemoryStorage::default());
}
