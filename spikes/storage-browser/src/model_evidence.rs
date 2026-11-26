//! SPEC-0026 / TC-0026: test-only public model conformance and persistence.
use super::*;
use hoyadb_core::{
    model::{Database, Migration},
    storage::{ModelCatalog, SchemaStorage},
};

#[path = "../../../tests/conformance/data_model.rs"]
mod model_cases;
#[path = "../../../tests/conformance/model_restart.rs"]
mod restart_cases;
#[path = "../../../tests/conformance/schema_storage.rs"]
mod schema_cases;
fn run_local<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous model fixture suspended"),
    }
}

#[wasm_bindgen]
pub fn model_conformance() -> Array {
    let cases: [(&str, fn()); 20] = [
        ("S01", schema_cases::s01_initialize_catalog_and_owned_scan),
        ("S02", schema_cases::s02_failed_prefix_preserves_all_state),
        (
            "S03",
            schema_cases::s03_stale_expected_state_rejects_before_operations,
        ),
        (
            "S04",
            schema_cases::s04_unknown_bookkeeping_and_corrupt_metadata_never_become_absent,
        ),
        (
            "S05",
            schema_cases::s05_checkpoints_and_final_candidate_validate_before_commit,
        ),
        (
            "D01",
            model_cases::d01_declared_schema_typed_records_and_bookkeeping,
        ),
        (
            "D02",
            model_cases::d02_invalid_declarations_and_reserved_inputs_leave_state_unchanged,
        ),
        (
            "D03",
            model_cases::d03_generated_ids_public_mutations_and_counter_rollback,
        ),
        (
            "D04",
            model_cases::d04_public_operators_all_kinds_order_and_limit,
        ),
        (
            "D05",
            model_cases::d05_lazy_to_one_has_many_and_engine_owned_fetch,
        ),
        (
            "D06",
            model_cases::d06_many_to_many_managed_links_and_both_directions,
        ),
        ("D07", model_cases::d07_identical_reopen_and_schema_drift),
        (
            "D08",
            model_cases::d08_initialization_and_compatible_legacy_adoption,
        ),
        (
            "D09",
            model_cases::d09_complete_authored_chain_applies_all_primitives,
        ),
        (
            "D10",
            model_cases::d10_failed_migration_and_endpoint_return_original_owned_engine,
        ),
        (
            "D11",
            model_cases::d11_missing_complete_chain_resets_without_running_prefix,
        ),
        (
            "D12",
            model_cases::d12_downgrade_unknown_queue_and_future_metadata_reject,
        ),
        (
            "D13",
            model_cases::d13_definition_codec_and_bound_unicode_names_roundtrip,
        ),
        (
            "D14",
            model_cases::d14_public_boundary_partitions_preserve_managed_state,
        ),
        (
            "TC0028",
            model_cases::tc0028_unselected_migration_descriptors_validate_nested_names,
        ),
    ];
    let passed = Array::new();
    for (name, test) in cases {
        test();
        cleanup_conformance();
        passed.push(&name.into());
    }
    passed
}

fn connection(name: &str) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags_and_vfs(
        name,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
        "opfs-sahpool",
    )
    .unwrap()
}

fn install(name: &str) {
    connection(name).execute_batch("CREATE TRIGGER abort_model BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_model' BEGIN SELECT RAISE(ABORT,'fixture metadata write failure'); END").unwrap();
}

fn release(name: &str) {
    connection(name)
        .execute_batch("DROP TRIGGER abort_model")
        .unwrap();
}

fn recover(
    result: Result<Database<SqliteStorage>, (StorageError, SqliteStorage)>,
) -> SqliteStorage {
    match result {
        Err((error, store)) => {
            assert!(matches!(error, StorageError::BackendFailure { .. }));
            store
        }
        Ok(_) => panic!("fixture SQL error unexpectedly committed"),
    }
}

#[wasm_bindgen]
pub fn model_physical() -> Array {
    let passed = Array::new();
    for (index, adopt) in [false, true].into_iter().enumerate() {
        let name = format!("model-initial-{index}.sqlite");
        {
            let mut store = SqliteStorage::open_unprotected(&name).unwrap();
            if adopt {
                run(store.create_table("customers")).unwrap();
                run(store.insert(
                    "customers",
                    Row {
                        id: "legacy".into(),
                        fields: BTreeMap::from([("country".into(), Value::String("FR".into()))]),
                    },
                ))
                .unwrap();
            }
        }
        install(&name);
        let store = recover(run_local(Database::open_recoverable(
            SqliteStorage::open_unprotected(&name).unwrap(),
            restart_cases::schema(1),
            vec![],
        )));
        assert_eq!(
            run(store.catalog()).unwrap(),
            ModelCatalog {
                state: None,
                tables: if adopt {
                    vec!["customers".into()]
                } else {
                    vec![]
                }
            }
        );
        if adopt {
            assert_eq!(
                run(store.scan("customers")).unwrap(),
                vec![Row {
                    id: "legacy".into(),
                    fields: BTreeMap::from([("country".into(), Value::String("FR".into()))])
                }]
            );
        }
        drop(store);
        release(&name);
        drop(
            run_local(Database::open(
                SqliteStorage::open_unprotected(&name).unwrap(),
                restart_cases::schema(1),
                vec![],
            ))
            .unwrap(),
        );
        passed.push(
            &if adopt {
                "D15 adoption rollback"
            } else {
                "D15 initialization rollback"
            }
            .into(),
        );
    }

    const INSERT: &str = "model-insert-failure.sqlite";
    drop(
        run_local(Database::open(
            SqliteStorage::open_unprotected(INSERT).unwrap(),
            restart_cases::schema(1),
            vec![],
        ))
        .unwrap(),
    );
    install(INSERT);
    {
        let mut db = run_local(Database::open(
            SqliteStorage::open_unprotected(INSERT).unwrap(),
            restart_cases::schema(1),
            vec![],
        ))
        .unwrap();
        assert!(matches!(
            run_local(db.insert(
                "customers",
                BTreeMap::from([("country".into(), Value::String("FR".into()))])
            )),
            Err(StorageError::BackendFailure { .. })
        ));
        let store = db.into_storage();
        assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 1);
        assert_eq!(run(store.scan("customers")), Ok(vec![]));
    }
    release(INSERT);
    {
        let mut db = run_local(Database::open(
            SqliteStorage::open_unprotected(INSERT).unwrap(),
            restart_cases::schema(1),
            vec![],
        ))
        .unwrap();
        assert_eq!(
            run_local(db.insert(
                "customers",
                BTreeMap::from([("country".into(), Value::String("FR".into()))])
            ))
            .unwrap()
            .id,
            "local:1"
        );
    }
    passed.push(&"D15 public row and counter rollback".into());
    for (name, version, migrations) in [
        (
            "model-migration-failure.sqlite",
            3,
            vec![Migration {
                from_version: 2,
                to: restart_cases::schema(3),
                steps: vec![],
            }],
        ),
        ("model-reset-failure.sqlite", 4, vec![]),
    ] {
        run_local(restart_cases::write(
            SqliteStorage::open_unprotected(name).unwrap(),
        ))
        .unwrap();
        install(name);
        let store = recover(run_local(Database::open_recoverable(
            SqliteStorage::open_unprotected(name).unwrap(),
            restart_cases::schema(version),
            migrations.clone(),
        )));
        run_local(restart_cases::read(store, false)).unwrap();
        release(name);
        let db = run_local(Database::open(
            SqliteStorage::open_unprotected(name).unwrap(),
            restart_cases::schema(version),
            migrations,
        ))
        .unwrap();
        let store = db.into_storage();
        assert_eq!(
            run(store.catalog()).unwrap().state.unwrap().version,
            version
        );
        if version == 4 {
            assert_eq!(run(store.scan("orders")), Ok(vec![]));
        }
        drop(store);
        let c = connection(name);
        assert_eq!(
            c.query_row("SELECT version FROM __hoya_meta", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(c);
        passed.push(
            &if version == 3 {
                "D15 migration rollback"
            } else {
                "D15 reset rollback"
            }
            .into(),
        );
    }
    passed
}

#[wasm_bindgen]
pub fn model_fixture_write() -> Result<(), JsValue> {
    run_local(restart_cases::write(
        SqliteStorage::open_unprotected("model-restart.sqlite").map_err(error)?,
    ))
    .map_err(error)
}

#[wasm_bindgen]
pub fn model_fixture_read(advance: bool) -> Result<JsValue, JsValue> {
    let (rows, next) = run_local(restart_cases::read(
        SqliteStorage::open_unprotected("model-restart.sqlite").map_err(error)?,
        advance,
    ))
    .map_err(error)?;
    // Complete rows and relation/metadata contents were asserted in the shared
    // Rust reader above. Return the actual owned IDs for page-level correlation.
    Ok(object(&[
        (
            "rowIds",
            rows.into_iter()
                .map(|row| JsValue::from(row.id))
                .collect::<Array>()
                .into(),
        ),
        (
            "nextId",
            next.map(|row| JsValue::from(row.id))
                .unwrap_or(JsValue::NULL),
        ),
    ]))
}

// Shared observation cases import their existing public model fixtures by this alias.
use model_cases as model_conformance;

#[path = "../../../tests/conformance/projection.rs"]
mod projection_cases;
#[path = "../../../tests/conformance/reactivity.rs"]
mod reactive_cases;
pub(super) fn reactive_schema() -> hoyadb_core::model::SchemaDefinition {
    model_cases::schema(1)
}

#[wasm_bindgen]
pub fn reactive_conformance() -> Array {
    let cases: &[(&str, fn())] = &[
        ("R01", reactive_cases::r01_owned_initial_and_membership_only),
        (
            "R02",
            reactive_cases::r02_cancel_close_and_cross_owner_handles,
        ),
        (
            "R03",
            reactive_cases::r03_net_batches_rollback_and_commit_snapshots,
        ),
        (
            "R04",
            reactive_cases::r04_own_columns_missing_null_and_registration_validation,
        ),
        (
            "R05",
            reactive_cases::r05_count_throttle_deadline_return_and_backwards_clock,
        ),
        (
            "R06",
            reactive_cases::r06_normalized_equivalent_queries_share_across_modes,
        ),
        (
            "R07",
            reactive_cases::r07_ordered_limit_refill_ties_and_reorder_suppression,
        ),
        (
            "R08",
            reactive_cases::r08_to_one_dependencies_dangling_and_unrelated_writes,
        ),
        (
            "R09",
            reactive_cases::r09_all_relation_navigations_track_source_target_and_pivots,
        ),
        (
            "R10",
            reactive_cases::r10_postcommit_faults_registration_and_explicit_recovery,
        ),
        (
            "R11 values",
            projection_cases::r11_all_values_owned_and_only_requested_cells,
        ),
        (
            "R11 operators",
            projection_cases::r11_filter_order_limit_and_nulls,
        ),
        (
            "R11 relations",
            projection_cases::r11_direct_relations_missing_targets_and_cells,
        ),
        (
            "R11 validation",
            projection_cases::r11_validation_before_zero_limit_and_unrequested_rows_stay_unchecked,
        ),
        (
            "R11 names",
            projection_cases::r11_bound_names_and_required_cells,
        ),
        ("R12", reactive_cases::r12_noop_link_unlink_and_reopen),
        (
            "R15",
            reactive_cases::r15_count_work_scales_with_affected_ids_and_shared_observers,
        ),
    ];
    let passed = Array::new();
    for (name, test) in cases {
        test();
        cleanup_conformance();
        passed.push(&(*name).into());
    }
    passed
}

/// SPEC-0028 / TC-0031: actual worker regressions and unchanged pooled inventory.
#[wasm_bindgen]
pub fn review_conformance(index: u32) -> Result<JsValue, JsValue> {
    let cases: &[(&str, fn())] = &[
        (
            "Q01 large observations/navigation/reverse dependencies",
            reactive_cases::review_q01_large_result_sets_and_navigation,
        ),
        (
            "Q01 owner TEMP window/intersection",
            reactive_cases::review_q01_owner_temp_window_intersection,
        ),
        (
            "Q01 oversized all-kind/Null/AND parity",
            projection_cases::review_q01_oversized_operands_preserve_all_types,
        ),
        (
            "Q02 repeated explicit retry errors",
            reactive_cases::review_q02_explicit_retry_errors_are_observable,
        ),
        (
            "Q03 row/count failure isolation",
            reactive_cases::review_q03_row_failure_keeps_count_live,
        ),
    ];
    let Some(case) = cases.get(index as usize) else {
        return Err(backend("unknown review case"));
    };
    let passed = Array::new();
    let pools = Array::new();
    let files = || {
        POOL.with(|slot| {
            let mut files = slot
                .borrow()
                .as_ref()
                .unwrap()
                .names()
                .expect("active owned pool");
            files.sort();
            files
        })
    };
    for (name, test) in std::iter::once(case) {
        let before = files();
        test();
        cleanup_conformance();
        let after = files();
        assert_eq!(after, before, "query operands cannot add pooled files");
        passed.push(&(*name).into());
        pools.push(&object(&[
            ("case", (*name).into()),
            (
                "before",
                before
                    .into_iter()
                    .map(JsValue::from)
                    .collect::<Array>()
                    .into(),
            ),
            (
                "after",
                after
                    .into_iter()
                    .map(JsValue::from)
                    .collect::<Array>()
                    .into(),
            ),
        ]));
    }
    Ok(object(&[
        ("cases", passed.into()),
        ("poolChecks", pools.into()),
    ]))
}
