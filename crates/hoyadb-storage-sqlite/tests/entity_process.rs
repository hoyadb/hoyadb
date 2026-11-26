//! TC-0034: each restart step is a new process over the real SQLite file.
#[test]
fn e04_e07_e13_e17_independent_entity_pull_and_reset_processes() {
    let directory =
        std::env::temp_dir().join(format!("hoya-entity-process-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    for mode in ["write", "read", "reset", "read-reset"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_entity_restart_fixture"))
            .arg(mode)
            .arg(directory.join("store.sqlite"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("PHASE5_PROCESS_{mode}_GREEN"))
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
