//! API-0011 §7: complete success-frame sizing before durable mutations.
use crate::{Result, codec::*, data::*};
use hoyadb_core::{
    command::Json,
    model::Write,
    storage::{Row, Value},
};
use std::collections::BTreeMap;

pub(crate) fn success(value: &Json) -> Result<()> {
    object([
        ("abi", text("1")),
        ("ok", Json::Bool(true)),
        ("value", value.clone()),
    ])
    .canonical()?;
    Ok(())
}

/// One owned reply prefix for a keyed readonly list (API-0011 §4 paged reads).
/// `after` names the last item the caller already holds; an unknown cursor is
/// Protocol. Byte arithmetic selects the longest candidate prefix once; the
/// complete frame check stays authoritative and only ever shortens it. A first
/// item that cannot be a sole page item is LimitExceeded, never skipped.
pub(crate) fn page(
    items: Vec<(String, Json)>,
    after: Option<&str>,
    frame: impl Fn(Vec<Json>, bool) -> Json,
) -> Result<Json> {
    let start = match after {
        None => 0,
        Some(key) => {
            items
                .iter()
                .position(|(candidate, _)| candidate == key)
                .ok_or(crate::Error("Protocol"))?
                + 1
        }
    };
    let items: Vec<Json> = items.into_iter().skip(start).map(|(_, v)| v).collect();
    let reply = |value: &Json| {
        object([
            ("abi", text("1")),
            ("ok", Json::Bool(true)),
            ("value", value.clone()),
        ])
    };
    let mut bytes = reply(&frame(vec![], false)).canonical()?.len();
    let mut count = 0;
    for item in &items {
        // The separator of the first item is charged too; the bound stays safe.
        bytes = bytes.saturating_add(item.canonical()?.len() + 1);
        if bytes > 1_048_576 + 1 {
            break;
        }
        count += 1;
    }
    while count > 0 {
        let value = frame(items[..count].to_vec(), count < items.len());
        if success(&value).is_ok() {
            return Ok(value);
        }
        count /= 2;
    }
    if !items.is_empty() {
        return Err(crate::Error("LimitExceeded"));
    }
    let value = frame(vec![], false);
    success(&value)?;
    Ok(value)
}

pub(crate) fn row(id: Option<&str>, fields: &BTreeMap<String, Value>) -> Result<()> {
    success(&row_wire(&Row {
        id: id
            .unwrap_or("entity:ffffffffffffffffffffffffffffffff:ffffffffffffffff")
            .into(),
        fields: fields.clone(),
    }))
}

// ABI1's complete success frame is limited to 1 MiB. These independently
// encoded parts are a lower bound only; the complete serializer stays authoritative.
#[derive(Default)]
pub(crate) struct AliasedReplyLowerBound {
    bytes: usize,
}
impl AliasedReplyLowerBound {
    fn add(&mut self, value: &Json) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(value.canonical()?.len())
            .ok_or(crate::Error("LimitExceeded"))?;
        if self.bytes > 1_048_576 {
            return Err(crate::Error("LimitExceeded"));
        }
        Ok(())
    }
    pub(crate) fn after(&mut self, write: &Write, proposed: Option<&str>) -> Result<()> {
        let (id, fields) = match write {
            Write::Insert {
                fields,
                ..
            } => (
                proposed.unwrap_or("entity:ffffffffffffffffffffffffffffffff:ffffffffffffffff"),
                fields,
            ),
            Write::Replace {
                id,
                fields,
                ..
            } => (id.as_str(), fields),
            Write::Remove {
                ..
            } => return Ok(()),
        };
        self.add(&row_wire(&Row {
            id: id.into(),
            fields: fields.clone(),
        }))
    }
}

pub(crate) fn batch(
    writes: &[(Write, Option<String>)],
    before: impl FnMut(&str, &str) -> Result<Vec<Row>>,
) -> Result<()> {
    batch_report(writes, before, None)
}

pub(crate) fn aliased_batch(
    writes: &[(Write, Option<String>)],
    before: impl FnMut(&str, &str) -> Result<Vec<Row>>,
    aliases: &BTreeMap<String, String>,
) -> Result<()> {
    batch_report(writes, before, Some(aliases))
}

fn batch_report(
    writes: &[(Write, Option<String>)],
    mut before: impl FnMut(&str, &str) -> Result<Vec<Row>>,
    aliases: Option<&BTreeMap<String, String>>,
) -> Result<()> {
    let report = |changes: Vec<Json>| {
        let changes = Json::Array(changes);
        if let Some(aliases) = aliases {
            success(&object([
                ("changes", changes),
                (
                    "aliases",
                    Json::Object(
                        aliases
                            .iter()
                            .map(|(alias, id)| (alias.clone(), text(id)))
                            .collect(),
                    ),
                ),
            ]))
        } else {
            success(&changes)
        }
    };
    let mut changes = Vec::new();
    let mut lower_bound = AliasedReplyLowerBound::default();
    let mut known: BTreeMap<(String, String), Option<Row>> = BTreeMap::new();
    // Rows inserted under a core-allocated ID that no later read has charged yet.
    let mut unknown: Vec<(String, Row)> = Vec::new();
    for (write, proposed) in writes {
        let allocated = matches!(write, Write::Insert { .. }) && proposed.is_none();
        let (table, id, after, read) = match write {
            Write::Insert {
                table,
                fields,
            } => {
                let id = proposed
                    .as_deref()
                    .unwrap_or("entity:ffffffffffffffffffffffffffffffff:ffffffffffffffff");
                (
                    table,
                    id,
                    Some(Row {
                        id: id.into(),
                        fields: fields.clone(),
                    }),
                    false,
                )
            }
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
        // The before image is the target's own current row. A row inserted
        // earlier under an ID only core knows can be the before image of at
        // most one later target (its ID is then known), so it is charged once,
        // to the first later read of its table. The sizing frame is therefore
        // never smaller than the real reply in bytes or nodes; it is never a
        // returned change or replacement mutation.
        let mut images = Vec::new();
        if read {
            match known.get(&(table.clone(), id.to_owned())) {
                Some(row) => images.extend(row.iter().map(row_wire)),
                None => images.extend(before(table, id)?.iter().map(row_wire)),
            }
            let mut index = 0;
            while index < unknown.len() {
                if &unknown[index].0 == table {
                    images.push(row_wire(&unknown.remove(index).1));
                } else {
                    index += 1;
                }
            }
        }
        let before_image = match images.len() {
            0 => Json::Null,
            1 => images.remove(0),
            _ => Json::Array(images),
        };
        changes.push(object([
            ("table", text(table)),
            ("id", text(id)),
            ("before", before_image),
            ("after", after.as_ref().map(row_wire).unwrap_or(Json::Null)),
        ]));
        // Each change is encoded once here; their sum is a lower bound of the
        // complete frame, so reading stops as soon as the reply cannot fit.
        lower_bound.add(changes.last().expect("sizing change"))?;
        if allocated {
            if let Some(after) = after {
                unknown.push((table.clone(), after));
            }
        } else {
            known.insert((table.clone(), id.to_owned()), after);
        }
    }
    report(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliased_before_images_stop_reading_after_complete_change_lower_bound() {
        let writes = (0..5)
            .map(|index| {
                (
                    Write::Remove {
                        table: "items".into(),
                        id: index.to_string(),
                    },
                    None,
                )
            })
            .collect::<Vec<_>>();
        let mut reads = 0;
        let error = aliased_batch(
            &writes,
            |_, id| {
                reads += 1;
                Ok(vec![Row {
                    id: id.into(),
                    fields: BTreeMap::from([("body".into(), Value::String("x".repeat(300_000)))]),
                }])
            },
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(error.0, "LimitExceeded");
        assert_eq!(reads, 4, "stop before retaining a fifth large before image");
    }

    /// A prefix that fits in bytes but not in nodes is shortened by the complete
    /// frame check, and following the cursor still returns every item once, in order.
    #[test]
    fn page_cursor_walk_returns_every_item_once_within_node_and_byte_limits() {
        let items = || {
            (0..30)
                .map(|index| {
                    (
                        format!("k{index}"),
                        object([
                            ("key", text(format!("k{index}"))),
                            ("wide", Json::Array(vec![Json::Null; 5_000])),
                        ]),
                    )
                })
                .collect::<Vec<_>>()
        };
        let frame = |entries: Vec<Json>, more: bool| {
            object([
                ("entries", Json::Array(entries)),
                ("has_more", Json::Bool(more)),
            ])
        };
        let (mut seen, mut after, mut pages) = (Vec::new(), None::<String>, 0);
        loop {
            let value = page(items(), after.as_deref(), frame).unwrap();
            success(&value).unwrap();
            let fields = map(&value).unwrap();
            let entries = array(&fields["entries"]).unwrap();
            assert!(!entries.is_empty());
            for entry in entries {
                seen.push(string(&map(entry).unwrap()["key"]).unwrap().to_owned());
            }
            pages += 1;
            if fields["has_more"] == Json::Bool(false) {
                break;
            }
            after = seen.last().cloned();
        }
        assert!(pages > 1);
        assert_eq!(
            seen,
            (0..30).map(|index| format!("k{index}")).collect::<Vec<_>>()
        );
        assert_eq!(
            page(items(), Some("gone"), frame).unwrap_err().0,
            "Protocol"
        );
        let huge = vec![("k".to_owned(), text("x".repeat(1_048_576)))];
        assert_eq!(page(huge, None, frame).unwrap_err().0, "LimitExceeded");
    }

    fn body(bytes: usize) -> BTreeMap<String, Value> {
        BTreeMap::from([("body".into(), Value::String("x".repeat(bytes)))])
    }

    /// One core-allocated insert can be the before image of at most one later
    /// target, so it is charged once, not to every later replace of its table.
    #[test]
    fn allocated_insert_is_charged_once_across_later_reads_of_its_table() {
        let mut writes = vec![(
            Write::Insert {
                table: "items".into(),
                fields: body(400_000),
            },
            None,
        )];
        for id in ["x", "y", "z"] {
            writes.push((
                Write::Replace {
                    table: "items".into(),
                    id: id.into(),
                    fields: body(1),
                },
                None,
            ));
        }
        let mut reads = 0;
        batch(&writes, |_, id| {
            reads += 1;
            Ok(vec![Row {
                id: id.into(),
                fields: body(1),
            }])
        })
        .expect("a reply of about 400 KiB fits one frame");
        assert_eq!(reads, 3);
    }

    /// The single charge still covers the write that may target the allocation.
    #[test]
    fn allocated_insert_still_bounds_the_one_read_that_may_target_it() {
        let writes = vec![
            (
                Write::Insert {
                    table: "items".into(),
                    fields: body(600_000),
                },
                None,
            ),
            (
                Write::Remove {
                    table: "other".into(),
                    id: "o".into(),
                },
                None,
            ),
            (
                Write::Remove {
                    table: "items".into(),
                    id: "local:1".into(),
                },
                None,
            ),
        ];
        let error = batch(&writes, |_, _| Ok(vec![])).unwrap_err();
        assert_eq!(error.0, "LimitExceeded");
    }

    /// Sizing is one pass: each before image is read and encoded once.
    #[test]
    fn many_small_writes_read_each_target_once() {
        let mut writes = Vec::new();
        for index in 0..1_000 {
            writes.push((
                Write::Insert {
                    table: "items".into(),
                    fields: body(1),
                },
                None,
            ));
            writes.push((
                Write::Remove {
                    table: "items".into(),
                    id: format!("row-{index}"),
                },
                None,
            ));
        }
        let mut reads = 0;
        batch(&writes, |_, _| {
            reads += 1;
            Ok(vec![])
        })
        .expect("a small reply fits");
        assert_eq!(reads, 1_000);
    }
}
