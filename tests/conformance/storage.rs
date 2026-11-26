//! TC-0019 M01–M09 / TC-0022 / API-0002: shared storage subset assertions.
//! These tests do not instrument filesystem or platform API access.
use super::fresh;
use hoyadb_core::storage::{Mutation, TransactionalStorage};
use hoyadb_core::storage::{Row, Storage, StorageError, Value};
use std::collections::BTreeMap;
use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::task::Wake;
use std::task::{Context, Poll, Waker};

#[cfg(not(target_arch = "wasm32"))]
struct ThreadWake(std::thread::Thread);

#[cfg(not(target_arch = "wasm32"))]
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn run<F: Future + Send>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    #[cfg(not(target_arch = "wasm32"))]
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    #[cfg(not(target_arch = "wasm32"))]
    let waker = &waker;
    #[cfg(target_arch = "wasm32")]
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    #[cfg(not(target_arch = "wasm32"))]
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::park(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("SQLite operations must return ready Send futures"),
    }
}

fn order(id: &str, total: i64, paid: bool) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([
            ("status".into(), Value::String("保留 🌿".into())),
            ("total_cents".into(), Value::Integer(total)),
            ("paid".into(), Value::Boolean(paid)),
        ]),
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m01_typed_roundtrip() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    for row in [
        order("é-min", i64::MIN, false),
        order("🌿-max", i64::MAX, true),
    ] {
        run(engine.insert("orders", row.clone())).unwrap();
        assert_eq!(run(engine.get("orders", &row.id)), Ok(Some(row)));
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m02_reads_are_owned_snapshots() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("a", 1200, false);
    run(engine.insert("orders", original.clone())).unwrap();
    let mut snapshot = run(engine.get("orders", "a")).unwrap().unwrap();
    snapshot.id = "changed".into();
    snapshot.fields.clear();
    drop(snapshot);
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(original)));
    assert_eq!(run(engine.get("orders", "changed")), Ok(None));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
pub fn m03_tables_and_instances_are_isolated() {
    let mut first = fresh();
    let mut second = fresh();
    for engine in [&mut first, &mut second] {
        run(engine.create_table("orders")).unwrap();
        run(engine.create_table("other")).unwrap();
    }
    let a = order("same", 1, false);
    let b = order("same", 2, true);
    let c = order("same", 3, false);
    run(first.insert("orders", a.clone())).unwrap();
    run(first.insert("other", b.clone())).unwrap();
    run(second.insert("orders", c.clone())).unwrap();
    assert_eq!(run(first.get("orders", "same")), Ok(Some(a)));
    assert_eq!(run(first.get("other", "same")), Ok(Some(b)));
    assert_eq!(run(second.get("orders", "same")), Ok(Some(c)));
    assert_eq!(run(second.get("other", "same")), Ok(None));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m04_duplicate_insert_preserves_existing_row() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("a", 1, false);
    run(engine.insert("orders", original.clone())).unwrap();
    assert_eq!(
        run(engine.insert("orders", order("a", 2, true))),
        Err(StorageError::RowAlreadyExists {
            table: "orders".into(),
            id: "a".into()
        })
    );
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(original)));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m05_unknown_table_rejects_every_operation() {
    let mut engine = fresh();
    let error = StorageError::TableNotFound {
        table: "missing".into(),
    };
    assert_eq!(
        run(engine.insert("missing", order("a", 1, false))),
        Err(error.clone())
    );
    assert_eq!(run(engine.get("missing", "a")), Err(error.clone()));
    assert_eq!(
        run(engine.replace("missing", order("a", 2, true))),
        Err(error.clone())
    );
    assert_eq!(run(engine.remove("missing", "a")), Err(error.clone()));
    assert_eq!(run(engine.get("missing", "a")), Err(error));
    run(engine.create_table("missing")).unwrap();
    assert_eq!(run(engine.get("missing", "a")), Ok(None));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m06_missing_read_and_replacement() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    assert_eq!(run(engine.get("orders", "a")), Ok(None));
    assert_eq!(
        run(engine.replace("orders", order("a", 1, false))),
        Err(StorageError::RowNotFound {
            table: "orders".into(),
            id: "a".into()
        })
    );
    assert_eq!(run(engine.get("orders", "a")), Ok(None));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m07_replacement_replaces_all_fields() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    run(engine.insert("orders", order("a", 1, false))).unwrap();
    let replacement = Row {
        id: "a".into(),
        fields: BTreeMap::from([("paid".into(), Value::Boolean(true))]),
    };
    run(engine.replace("orders", replacement.clone())).unwrap();
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(replacement)));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m08_removal_reports_existing_and_missing_rows() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    run(engine.insert("orders", order("a", 1, true))).unwrap();
    assert_eq!(run(engine.remove("orders", "a")), Ok(true));
    assert_eq!(run(engine.get("orders", "a")), Ok(None));
    assert_eq!(run(engine.remove("orders", "a")), Ok(false));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn m09_creating_existing_table_preserves_rows() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("a", 1, true);
    run(engine.insert("orders", original.clone())).unwrap();
    run(engine.create_table("orders")).unwrap();
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(original)));
}

// TC-0020/TC-0022 T01–T07: ordered atomic writes, not queue durability.
#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t01_mixed_cross_table_batch_commits() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    run(engine.create_table("audit")).unwrap();
    let old = order("old", 1, false);
    let target = order("target", 2, false);
    let keeper = order("keeper", 3, true);
    for row in [old, target] {
        run(engine.insert("orders", row)).unwrap();
    }
    run(engine.insert("audit", keeper.clone())).unwrap();
    let added = order("added", i64::MAX, true);
    let replaced = order("target", i64::MIN, true);
    let stamp = order("stamp", 4, false);
    run(engine.apply_batch(vec![
        Mutation::Insert {
            table: "orders".into(),
            row: added.clone(),
        },
        Mutation::Replace {
            table: "orders".into(),
            row: replaced.clone(),
        },
        Mutation::Insert {
            table: "audit".into(),
            row: stamp.clone(),
        },
        Mutation::Remove {
            table: "orders".into(),
            id: "old".into(),
        },
    ]))
    .unwrap();
    assert_eq!(run(engine.get("orders", "added")), Ok(Some(added)));
    assert_eq!(run(engine.get("orders", "target")), Ok(Some(replaced)));
    assert_eq!(run(engine.get("orders", "old")), Ok(None));
    assert_eq!(run(engine.get("audit", "stamp")), Ok(Some(stamp)));
    assert_eq!(run(engine.get("audit", "keeper")), Ok(Some(keeper)));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t02_batch_observes_prior_mutations_in_order() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let final_row = order("a", 3, true);
    run(engine.apply_batch(vec![
        Mutation::Insert {
            table: "orders".into(),
            row: order("a", 1, false),
        },
        Mutation::Replace {
            table: "orders".into(),
            row: order("a", 2, false),
        },
        Mutation::Remove {
            table: "orders".into(),
            id: "a".into(),
        },
        Mutation::Insert {
            table: "orders".into(),
            row: final_row.clone(),
        },
    ]))
    .unwrap();
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(final_row)));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t03_duplicate_rolls_back_all_tables_and_later_mutations() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    run(engine.create_table("audit")).unwrap();
    let original = order("seed", 1, false);
    let audit = order("audit-seed", 2, true);
    run(engine.insert("orders", original.clone())).unwrap();
    run(engine.insert("audit", audit.clone())).unwrap();
    let result = run(engine.apply_batch(vec![
        Mutation::Insert {
            table: "orders".into(),
            row: order("added", 3, true),
        },
        Mutation::Replace {
            table: "orders".into(),
            row: order("seed", 4, true),
        },
        Mutation::Remove {
            table: "audit".into(),
            id: "audit-seed".into(),
        },
        Mutation::Insert {
            table: "orders".into(),
            row: order("seed", 5, true),
        },
        Mutation::Insert {
            table: "audit".into(),
            row: order("later", 6, false),
        },
    ]));
    assert_eq!(
        result,
        Err(StorageError::RowAlreadyExists {
            table: "orders".into(),
            id: "seed".into()
        })
    );
    assert_eq!(run(engine.get("orders", "seed")), Ok(Some(original)));
    assert_eq!(run(engine.get("audit", "audit-seed")), Ok(Some(audit)));
    assert_eq!(run(engine.get("orders", "added")), Ok(None));
    assert_eq!(run(engine.get("audit", "later")), Ok(None));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t04_missing_replacement_restores_removed_row() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("a", 1, false);
    run(engine.insert("orders", original.clone())).unwrap();
    assert_eq!(
        run(engine.apply_batch(vec![
            Mutation::Remove {
                table: "orders".into(),
                id: "a".into()
            },
            Mutation::Replace {
                table: "orders".into(),
                row: order("a", 2, true)
            },
        ])),
        Err(StorageError::RowNotFound {
            table: "orders".into(),
            id: "a".into()
        })
    );
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(original)));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t05_unknown_table_rolls_back_for_each_mutation_kind() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("a", 1, false);
    run(engine.insert("orders", original.clone())).unwrap();
    for invalid in [
        Mutation::Insert {
            table: "missing".into(),
            row: order("a", 2, true),
        },
        Mutation::Replace {
            table: "missing".into(),
            row: order("a", 3, true),
        },
        Mutation::Remove {
            table: "missing".into(),
            id: "a".into(),
        },
    ] {
        assert_eq!(
            run(engine.apply_batch(vec![
                Mutation::Replace {
                    table: "orders".into(),
                    row: order("a", 9, true)
                },
                invalid,
            ])),
            Err(StorageError::TableNotFound {
                table: "missing".into()
            })
        );
        assert_eq!(run(engine.get("orders", "a")), Ok(Some(original.clone())));
        assert_eq!(
            run(engine.get("missing", "a")),
            Err(StorageError::TableNotFound {
                table: "missing".into()
            })
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t06_first_typed_error_is_preserved() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("seed", 1, false);
    run(engine.insert("orders", original.clone())).unwrap();
    assert_eq!(
        run(engine.apply_batch(vec![
            Mutation::Insert {
                table: "orders".into(),
                row: order("added", 2, true)
            },
            Mutation::Replace {
                table: "orders".into(),
                row: order("absent", 3, true)
            },
            Mutation::Remove {
                table: "unknown".into(),
                id: "x".into()
            },
            Mutation::Insert {
                table: "orders".into(),
                row: order("seed", 4, true)
            },
        ])),
        Err(StorageError::RowNotFound {
            table: "orders".into(),
            id: "absent".into()
        })
    );
    assert_eq!(run(engine.get("orders", "seed")), Ok(Some(original)));
    assert_eq!(run(engine.get("orders", "added")), Ok(None));
    assert_eq!(run(engine.get("orders", "absent")), Ok(None));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn t07_empty_and_missing_removal_batches_preserve_rows() {
    let mut engine = fresh();
    run(engine.create_table("orders")).unwrap();
    let original = order("a", 1, true);
    run(engine.insert("orders", original.clone())).unwrap();
    assert_eq!(run(engine.apply_batch(Vec::new())), Ok(()));
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(original.clone())));
    assert_eq!(
        run(engine.apply_batch(vec![Mutation::Remove {
            table: "orders".into(),
            id: "absent".into()
        }])),
        Ok(())
    );
    assert_eq!(run(engine.get("orders", "a")), Ok(Some(original)));
}
