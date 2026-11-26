//! TC-0022 Q01/Q02: independent writer/reader OS processes.
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn restart(scenario: &str) {
    let directory = std::env::temp_dir().join(format!(
        "hoya-sqlite-process-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let fixture = Fixture(directory);
    for mode in ["write", "read"] {
        let output = Command::new(env!("CARGO_BIN_EXE_restart_fixture"))
            .args([mode, scenario])
            .arg(fixture.0.join("database.sqlite"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode} {scenario} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn q01_committed_rows_survive_process_restart() {
    restart("commit");
}

#[test]
fn q02_failed_batch_remains_rolled_back_after_restart() {
    restart("rollback");
}

#[test]
fn p11_new_typed_values_survive_process_restart() {
    restart("typed");
}

#[test]
fn d16_managed_schema_migration_relations_and_ids_survive_process_restart() {
    restart("model");
}
