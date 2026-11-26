//! A debug worker can never silently create release performance evidence.
#[test]
fn p01_debug_worker_refuses_performance_evidence() {
    if cfg!(debug_assertions) {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_hoyadb-benchmark"))
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("requires release")
        );
        assert!(output.stdout.is_empty());
    }
}
