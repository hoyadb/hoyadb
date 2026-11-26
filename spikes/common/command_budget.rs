//! API-0008: conservative complete-frame budgets before mutating fixture calls.
use super::*;

const DEFAULT_ID: &str = "local:9223372036854775807";

pub(super) fn success(value: &Json) -> Result<()> {
    object([("ok", Json::Bool(true)), ("value", value.clone())])
        .canonical()
        .map(|_| ())
        .map_err(|_| Error("LimitExceeded"))
}

pub(super) fn finish(result: &Json) -> Result<()> {
    success(&object([
        ("key", text("0".repeat(66))),
        ("result", boxed(result)),
    ]))
}

pub(super) fn row(id: Option<&str>, fields: &BTreeMap<String, Value>) -> Result<()> {
    success(&row_wire(&Row {
        id: id.unwrap_or(DEFAULT_ID).into(),
        fields: fields.clone(),
    }))
}

pub(super) fn batch(
    writes: &[Write],
    before: impl FnMut(&str, &str) -> Result<Vec<Row>>,
) -> Result<()> {
    batch_with_id(writes, DEFAULT_ID, before)
}

pub(super) fn entity_batch(
    writes: &[Write],
    before: impl FnMut(&str, &str) -> Result<Vec<Row>>,
) -> Result<()> {
    batch_with_id(
        writes,
        "entity:ffffffffffffffffffffffffffffffff:ffffffffffffffff",
        before,
    )
}

fn batch_with_id(
    writes: &[Write],
    generated_id: &str,
    mut before: impl FnMut(&str, &str) -> Result<Vec<Row>>,
) -> Result<()> {
    let mut changes = Vec::new();
    let mut prior: Vec<(String, Row)> = Vec::new();
    for write in writes {
        let (table, id, after, read) = match write {
            Write::Insert {
                table,
                fields,
            } => (
                table,
                generated_id,
                Some(Row {
                    id: generated_id.into(),
                    fields: fields.clone(),
                }),
                false,
            ),
            Write::Replace {
                table,
                id,
                fields,
            } => (
                table,
                id.as_str(),
                Some(Row {
                    id: id.clone(),
                    fields: fields.clone(),
                }),
                true,
            ),
            Write::Remove {
                table,
                id,
            } => (table, id.as_str(), None, true),
        };
        // A before-array is a sizing upper bound, never a returned change.
        // It covers both byte and node counts even if the largest byte image
        // differs from the image with the most fields. Check progressively
        // instead of allocating a quadratic candidate expansion unchecked.
        changes.push(object([
            ("table", text(table)),
            ("id", text(id)),
            ("before", Json::Array(vec![Json::Null])),
            ("after", after.as_ref().map(row_wire).unwrap_or(Json::Null)),
        ]));
        success(&Json::Array(changes.clone()))?;
        if read {
            let current = before(table, id)?;
            for candidate in current.iter().chain(
                prior
                    .iter()
                    .filter(|(name, _)| name == table)
                    .map(|(_, row)| row),
            ) {
                let Json::Object(change) = changes.last_mut().expect("sizing entry") else {
                    unreachable!()
                };
                let Json::Array(images) = change.get_mut("before").expect("sizing before array")
                else {
                    unreachable!()
                };
                images.push(row_wire(candidate));
                success(&Json::Array(changes.clone()))?;
            }
        }
        if let Some(after) = after {
            prior.push((table.clone(), after));
        }
    }
    Ok(())
}

pub(super) fn wire_reports(keys: &[String]) -> Result<()> {
    let reports = keys
        .iter()
        .map(|key| {
            object([
                ("key", text(key)),
                ("ok", Json::Bool(false)),
                ("error", object([("kind", text("UnknownLocalReference"))])),
            ])
        })
        .collect();
    success(&Json::Array(reports))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn fields(size: usize) -> BTreeMap<String, Value> {
        BTreeMap::from([("label".into(), Value::String("x".repeat(size)))])
    }

    #[test]
    fn h03_finish_and_row_preflight_the_complete_success_frame() {
        assert!(finish(&Json::String("small".into())).is_ok());
        assert!(finish(&Json::String("x".repeat(1_048_500))).is_err());
        assert!(row(None, &fields(8)).is_ok());
        assert!(row(Some("東京\0"), &fields(1_048_540)).is_err());
    }

    #[test]
    fn h03_batch_budget_includes_existing_and_same_batch_before_images() {
        let writes = vec![Write::Replace {
            table: "orders".into(),
            id: "existing".into(),
            fields: fields(8),
        }];
        assert!(
            batch(&writes, |table, id| {
                assert_eq!((table, id), ("orders", "existing"));
                Ok(vec![])
            })
            .is_ok()
        );
        let writes = vec![Write::Replace {
            table: "orders".into(),
            id: "existing".into(),
            fields: fields(600_000),
        }];
        assert!(
            batch(&writes, |_, _| Ok(vec![Row {
                id: "existing".into(),
                fields: fields(600_000)
            }]))
            .is_err()
        );
        let writes = (0..3)
            .map(|_| Write::Replace {
                table: "orders".into(),
                id: "existing".into(),
                fields: fields(300_000),
            })
            .collect::<Vec<_>>();
        assert!(batch(&writes, |_, _| Ok(vec![])).is_err());
    }

    #[test]
    fn h06_success_budget_checks_nodes_as_well_as_report_bytes() {
        assert!(wire_reports(&["one".into(), "two".into()]).is_ok());
        assert!(wire_reports(&vec!["k".into(); 20_000]).is_err());
        let nodes = Json::Array(vec![Json::Null; 99_999]);
        assert!(
            nodes.canonical().is_ok(),
            "value alone is within the codec node bound"
        );
        assert!(
            success(&nodes).is_err(),
            "the complete reply includes outer-frame nodes"
        );
    }

    #[test]
    fn h06_one_envelope_fits_even_at_the_largest_accepted_record_bound() {
        use hoyadb_core::command::records::*;
        let mut record = SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: format!("0000000000000000-{}-0000000000000001", "0".repeat(32)),
            command: "sample-v1".into(),
            args: Json::Object(BTreeMap::new()),
            context: object([
                ("identity", Json::Null),
                ("capturedAt", text("1970-01-01T00:00:00.000Z")),
            ]),
            schema_version: 1,
            order: 1,
            reference_paths: vec![],
            dependencies: std::collections::BTreeSet::new(),
            created: BTreeMap::new(),
            mappings: BTreeMap::new(),
            undo: vec![],
            state: SyncState::Queued,
            frozen: None,
            attempts: 0,
            due_ms: 0,
            outcome: None,
            rolled_back: false,
            reconciled: false,
        };
        let mut low = 0usize;
        let mut high = 1_048_576usize;
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            record.args = object([("padding", text("x".repeat(mid)))]);
            if record.check_send_size().is_ok() {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        record.args = object([("padding", text("x".repeat(low)))]);
        record.check_send_size().unwrap();
        let value = object([
            (
                "envelopes",
                Json::Array(vec![text(record.envelope().canonical().unwrap())]),
            ),
            ("next_due_ms", text(i64::MAX)),
            ("has_more", Json::Bool(false)),
        ]);
        assert!(
            success(&value).is_ok(),
            "core frozen-copy budget dominates a one-envelope reply"
        );
    }
}
