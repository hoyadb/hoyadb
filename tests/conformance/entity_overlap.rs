//! SPEC-0038 C4 / TC-0042 (O2 slice) RED and characterization cases. Only existing public
//! operations are used, so unchanged code compiles and fails by assertion where C4 changes
//! behavior. Every case returns its storage and the expected ledger rows for a restart check.
#[allow(dead_code)]
#[path = "command_recovery.rs"]
mod recovery;
use hoyadb_core::command::entity::{FieldStamp, RowMetadata};
use hoyadb_core::command::records::SyncLedger;
use hoyadb_core::model::*;
use hoyadb_core::{command::*, storage::*};
pub use recovery::run;
use recovery::{edit, force_sent};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

pub type Clock = Arc<AtomicI64>;
pub type Outcome<S> = (S, Vec<Row>);

fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
    }
}

/// `memo` is never written by any case: it stays absent with no FieldStamp (EO17's y).
fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "notes".into(),
            TableDefinition {
                columns: BTreeMap::from([
                    ("body".into(), column(ColumnType::String, false)),
                    ("extra".into(), column(ColumnType::String, true)),
                    ("memo".into(), column(ColumnType::String, true)),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

pub fn fields(pairs: &[(&str, Option<&str>)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                v.map_or(Value::Null, |v| Value::String(v.into())),
            )
        })
        .collect()
}

fn configure<S: SyncStorage>(db: &mut CommandDatabase<S>, clock: &Clock) {
    let captured = clock.clone();
    db.configure_entities(
        EntityConfig {
            scope: "overlap-fixture".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        move || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: captured.load(Ordering::Relaxed),
            })
        },
    )
    .unwrap();
}

fn open_with<S: SyncStorage>(storage: S, history: usize) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [43; 16],
        SyncOptions {
            history,
            ..SyncOptions::default()
        },
    ))
    .unwrap()
}

pub struct Fixture<S: SyncStorage> {
    pub db: CommandDatabase<S>,
    pub clock: Clock,
    history: usize,
    /// A merge hook is in-memory owner state: it is re-registered on every owner reopen.
    hooked: bool,
}

impl<S: SyncStorage> Fixture<S> {
    pub fn new(storage: S) -> Self {
        Self::with_history(storage, SyncOptions::default().history)
    }

    pub fn with_history(storage: S, history: usize) -> Self {
        let mut db = open_with(storage, history);
        let clock = Arc::new(AtomicI64::new(5));
        configure(&mut db, &clock);
        Self {
            db,
            clock,
            history,
            hooked: false,
        }
    }

    /// Registers the default-keeping merge hook on `notes`, also for every later reopen.
    pub fn hook(&mut self) {
        self.db.set_merge_hook("notes", keep_default).unwrap();
        self.hooked = true;
    }

    fn reopen(storage: S, clock: Clock, history: usize, hooked: bool) -> Self {
        let mut db = open_with(storage, history);
        configure(&mut db, &clock);
        let mut fixture = Self {
            db,
            clock,
            history,
            hooked: false,
        };
        if hooked {
            fixture.hook();
        }
        fixture
    }

    pub fn at(&self, time: i64) {
        self.clock.store(time, Ordering::Relaxed);
    }

    fn last(&self) -> String {
        self.db.queue_status().commands.last().unwrap().key.clone()
    }

    /// A row created at `time` and confirmed, so its stamps are `{time, true}`.
    pub fn row(&mut self, time: i64, values: &[(&str, Option<&str>)]) -> String {
        self.at(time);
        let row = run(self.db.insert("notes", fields(values))).unwrap();
        let create = recovery::keys(&self.db.prepare_push(time, 0).unwrap());
        self.db
            .receive_entity(
                "notes",
                &row.id,
                &create[0],
                Some(RemoteOutcome::Applied(Json::Null)),
            )
            .unwrap();
        run(self.db.reconcile(
            &create[0],
            ReconcilePlan {
                mappings: BTreeMap::from([(row.id.clone(), row.id.clone())]),
                writes: vec![],
            },
        ))
        .unwrap();
        row.id
    }

    pub fn patch(&mut self, time: i64, id: &str, values: &[(&str, Option<&str>)]) -> String {
        self.at(time);
        run(self.db.patch("notes", id, fields(values))).unwrap();
        self.last()
    }

    /// Replaces the row: every present field is declared, every missing field is removed.
    pub fn replace(&mut self, time: i64, id: &str, values: &[(&str, Option<&str>)]) -> String {
        self.at(time);
        run(self.db.replace("notes", id, fields(values))).unwrap();
        self.last()
    }

    pub fn push(&mut self, now: i64) -> Vec<String> {
        recovery::keys(&self.db.prepare_push(now, 0).unwrap())
    }

    pub fn outcome(&mut self, id: &str, key: &str, outcome: RemoteOutcome) {
        self.db
            .receive_entity("notes", id, key, Some(outcome))
            .unwrap();
    }

    pub fn confirm(&mut self, id: &str, key: &str) {
        self.outcome(id, key, RemoteOutcome::Applied(Json::Null));
        run(self.db.reconcile(key, ReconcilePlan::default())).unwrap();
    }

    /// Rejected and processed (an empty reconcile of the rejection).
    pub fn reject(&mut self, id: &str, key: &str) {
        self.outcome(id, key, RemoteOutcome::Rejected("no".into()));
        run(self.db.reconcile(key, ReconcilePlan::default())).unwrap();
    }

    /// Reopens through the storage handle and returns the exact persisted state.
    pub fn snapshot(self) -> (Self, State) {
        let Fixture {
            db,
            clock,
            history,
            hooked,
        } = self;
        let status = db.queue_status();
        let pending = db.pending_reconciliations();
        let notes = run(db.query("notes").fetch(&db)).unwrap();
        let storage = db.into_storage();
        let rows = storage.load_sync().unwrap().unwrap();
        let ledger = SyncLedger::from_rows(&rows).unwrap();
        (
            Self::reopen(storage, clock, history, hooked),
            State {
                rows,
                ledger,
                notes,
                status,
                pending,
            },
        )
    }

    /// Applies a storage-level fixture step (closing and reopening the owner around it).
    pub fn with_storage(self, step: impl FnOnce(S) -> S) -> Self {
        let Fixture {
            db,
            clock,
            history,
            hooked,
        } = self;
        Self::reopen(step(db.into_storage()), clock, history, hooked)
    }

    /// Ends the case: the storage plus the exact ledger rows a fresh connection must read.
    pub fn finish(self) -> Outcome<S> {
        let (fixture, state) = self.snapshot();
        (fixture.db.into_storage(), state.rows)
    }
}

pub struct State {
    pub rows: Vec<Row>,
    pub ledger: SyncLedger,
    pub notes: Vec<Row>,
    pub status: QueueStatus,
    pub pending: Vec<(String, RemoteOutcome)>,
}

impl State {
    pub fn current(&self, id: &str) -> RowMetadata {
        self.ledger.entity.as_ref().unwrap().metadata[&("notes".to_string(), id.to_string())]
            .clone()
    }

    pub fn row(&self, id: &str) -> Option<Row> {
        self.notes.iter().find(|r| r.id == id).cloned()
    }
}

pub fn stamp(time: i64, authoritative: bool) -> FieldStamp {
    FieldStamp {
        timestamp_ms: time,
        authoritative,
    }
}

/// A whole image: the public row (`None` is absence) and its whole metadata.
pub type Image = (Option<Row>, RowMetadata);

/// A whole `notes` image of `id`: only the listed fields (`None` is an explicit Null, an unlisted
/// field is absent) and exactly the listed stamps (an unlisted field has no FieldStamp).
pub fn img(id: &str, values: &[(&str, Option<&str>)], stamps: &[(&str, i64, bool)]) -> Image {
    (
        Some(Row {
            id: id.into(),
            fields: fields(values),
        }),
        RowMetadata {
            fields: stamps
                .iter()
                .map(|(name, time, authoritative)| (name.to_string(), stamp(*time, *authoritative)))
                .collect(),
            tombstone: None,
        },
    )
}

/// A record's single row and metadata change on `id`, before and after, compared whole.
pub fn exact(state: &State, key: &str, id: &str, before: Image, after: Image) {
    let record = &state.ledger.records[key];
    assert_eq!(
        (record.undo.len(), record.metadata_undo.len()),
        (1, 1),
        "{key}: one change pair"
    );
    let (undo, meta) = (&record.undo[0], &record.metadata_undo[0]);
    assert_eq!((undo.table.as_str(), undo.id.as_str()), ("notes", id));
    assert_eq!(
        (meta.table.as_str(), meta.record_id.as_str()),
        ("notes", id)
    );
    assert_eq!(
        (undo.before.clone(), meta.before.clone()),
        (before.0, Some(before.1)),
        "{key}: whole before image"
    );
    assert_eq!(
        (undo.after.clone(), meta.after.clone()),
        (after.0, Some(after.1)),
        "{key}: whole after image"
    );
}

/// The current row and metadata of `id`, compared whole.
pub fn current(state: &State, id: &str, image: Image) {
    assert_eq!(
        (state.row(id), state.current(id)),
        image,
        "whole current row and metadata"
    );
}

/// Every field of a pending record except its undo and metadata undo is byte-identical: frozen
/// envelope, args, descriptor and removed fields, key, order, attempts, state and context.
pub fn preserved(before: &State, after: &State, key: &str) {
    let actual = &after.ledger.records[key];
    let mut expected = before.ledger.records[key].clone();
    expected.undo = actual.undo.clone();
    expected.metadata_undo = actual.metadata_undo.clone();
    assert!(
        expected == *actual,
        "{key}: only undo and metadata undo change"
    );
}

/// On C4 success every record outside K and L is byte-identical, and none is added or removed.
pub fn others_identical(before: &State, after: &State, touched: &[&String]) {
    assert!(
        before.ledger.records.keys().eq(after.ledger.records.keys()),
        "no record added or removed"
    );
    for (key, record) in &before.ledger.records {
        if !touched.contains(&key) {
            assert!(
                after.ledger.records[key] == *record,
                "{key}: a record outside K and L is identical"
            );
        }
    }
}

/// Closes the owner, reopens its storage through `reopen` (a fresh connection on SQLite; the same
/// instance on memory, which has no restart) and proves the reopened owner reads exactly `state`.
pub fn restarted<S: SyncStorage>(
    f: Fixture<S>,
    state: &State,
    reopen: impl FnOnce(S) -> S,
) -> Fixture<S> {
    let (f, reread) = f.with_storage(reopen).snapshot();
    assert_eq!(
        reread.rows, state.rows,
        "restart reads the identical ledger"
    );
    assert_eq!(
        reread.notes, state.notes,
        "restart reads the identical rows"
    );
    assert!(reread.status == state.status && reread.pending == state.pending);
    f
}

/// A typed Update correction of `id`, each field at its own backend time.
pub fn typed(id: &str, values: &[(&str, &str, i64)]) -> ReconcilePlan {
    ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Entity {
            change: EntityChange {
                table: "notes".into(),
                record_id: id.into(),
                op: EntityOperation::Update,
                changes: values
                    .iter()
                    .map(|(k, v, _)| (k.to_string(), Value::String(v.to_string())))
                    .collect(),
                removed_fields: BTreeSet::new(),
                timestamp_ms: values.iter().map(|(_, _, t)| *t).max().unwrap_or(0),
                field_timestamps: values.iter().map(|(k, _, t)| (k.to_string(), *t)).collect(),
            },
        }],
    }
}

/// Runs a refused action and proves nothing changed: rows, ledger rows, status, pending outcomes,
/// and no data or outcome notification.
pub fn refused<S: SyncStorage>(
    fixture: Fixture<S>,
    watch: &str,
    error: CommandError,
    action: impl FnOnce(&mut CommandDatabase<S>) -> Result<(), CommandError>,
) -> Fixture<S> {
    let (mut fixture, before) = fixture.snapshot();
    let query = fixture.db.query("notes");
    let data = run(fixture.db.observe(&query)).unwrap();
    let outcome = fixture.db.observe_outcome(watch).unwrap();
    fixture.db.drain_events(0);
    assert_eq!(action(&mut fixture.db), Err(error));
    let events = fixture.db.drain_events(0);
    assert!(
        !events.iter().any(|e| matches!(e, CommandEvent::Data(n) if n.subscription == data)
            || matches!(e, CommandEvent::Outcome { subscription, .. } if *subscription == outcome)),
        "a refusal publishes nothing"
    );
    let (fixture, after) = fixture.snapshot();
    assert_eq!(after.rows, before.rows, "ledger rows byte-identical");
    assert_eq!(after.notes, before.notes, "rows identical");
    assert!(after.status == before.status && after.pending == before.pending);
    fixture
}

fn corrected<S: SyncStorage>(f: &mut Fixture<S>, id: &str, key: &str) {
    f.outcome(id, key, RemoteOutcome::Corrected(Json::Null));
}

/// A on `body` at `a_time`, a pending later E on `body` at `e_time`, both sent, A Corrected.
fn correction_over_pending<S: SyncStorage>(
    storage: S,
    a_time: i64,
    e_time: i64,
) -> (Fixture<S>, String, String, String) {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let a = f.patch(a_time, &w, &[("body", Some("A"))]);
    let e = f.patch(e_time, &w, &[("body", Some("E"))]);
    assert_eq!(
        f.push(e_time + 1),
        vec![a.clone(), e.clone()],
        "setup: both sent"
    );
    corrected(&mut f, &w, &a);
    (f, w, a, e)
}

// ---------------------------------------------------------------- behavioral RED (C4 success)
// Every success case first pins today's exact pre-action images with the same whole-image
// helpers (these pass on unchanged code), then asserts the C4 result whole.

/// EO2: the correction re-bases the pending later writer, which still wins; a later rollback of E
/// restores the correction.
pub fn eo2<S: SyncStorage>(storage: S) -> Outcome<S> {
    let (f, w, a, e) = correction_over_pending(storage, 10, 20);
    let (mut f, before) = f.snapshot();
    let pending = img(&w, &[("body", Some("E"))], &[("body", 20, false)]);
    let optimistic = img(&w, &[("body", Some("A"))], &[("body", 10, false)]);
    exact(&before, &e, &w, optimistic, pending.clone());
    current(&before, &w, pending.clone());
    assert_eq!(
        run(f.db.reconcile(&a, typed(&w, &[("body", "S", 15)]))),
        Ok(())
    );
    let (mut f, after) = f.snapshot();
    let corrected = img(&w, &[("body", Some("S"))], &[("body", 15, true)]);
    exact(&after, &e, &w, corrected.clone(), pending.clone());
    current(&after, &w, pending);
    preserved(&before, &after, &e);
    others_identical(&before, &after, &[&a, &e]);
    assert!(after.ledger.records[&a].reconciled);
    f.reject(&w, &e);
    assert_eq!(run(f.db.rollback(&e)), Ok(()));
    let (f, last) = f.snapshot();
    current(&last, &w, corrected);
    f.finish()
}

/// TC-0044 CP03: C4 completes before configured Discard can refuse entity recovery.
pub fn cp03_c4_before_policy<S: SyncStorage>(storage: S) -> Outcome<S> {
    let (f, w, a, e) = correction_over_pending(storage, 10, 20);
    let (mut f, before) = f.snapshot();
    f.db.set_conflict_policy(ConflictPolicy {
        default: ConflictReaction::Discard,
        commands: BTreeMap::new(),
    })
    .unwrap();
    assert_eq!(
        run(f
            .db
            .reconcile_resolving(&a, typed(&w, &[("body", "S", 15)]), vec![])),
        Ok(Resolution::Completed)
    );
    let (f, after) = f.snapshot();
    let pending = img(&w, &[("body", Some("E"))], &[("body", 20, false)]);
    let corrected = img(&w, &[("body", Some("S"))], &[("body", 15, true)]);
    exact(&after, &e, &w, corrected, pending.clone());
    current(&after, &w, pending);
    preserved(&before, &after, &e);
    others_identical(&before, &after, &[&a, &e]);
    assert!(after.ledger.records[&a].reconciled);
    f.finish()
}

/// EO-ORDER: equal clocks for A and E. The correction and promotion go only into eff0; E's images
/// and the current cell (clock 20 like A's) are never promoted. The store restarts (a fresh
/// connection on SQLite) before C4, after C4 before E is rejected and rolled back, and at the end
/// through the runner.
pub fn eo_order<S: SyncStorage>(storage: S, reopen: impl Fn(S) -> S) -> Outcome<S> {
    let (f, w, a, e) = correction_over_pending(storage, 20, 20);
    let (f, before) = f.snapshot();
    let pending = img(&w, &[("body", Some("E"))], &[("body", 20, false)]);
    let optimistic = img(&w, &[("body", Some("A"))], &[("body", 20, false)]);
    exact(&before, &e, &w, optimistic, pending.clone());
    current(&before, &w, pending.clone());
    // A restart before C4 too, so the reopen step is exercised on unchanged code.
    let mut f = restarted(f, &before, &reopen);
    assert_eq!(
        run(f.db.reconcile(&a, typed(&w, &[("body", "S", 15)]))),
        Ok(())
    );
    let (f, after) = f.snapshot();
    let corrected = img(&w, &[("body", Some("S"))], &[("body", 15, true)]);
    exact(&after, &e, &w, corrected.clone(), pending.clone());
    current(&after, &w, pending);
    preserved(&before, &after, &e);
    others_identical(&before, &after, &[&a, &e]);
    assert!(after.ledger.records[&a].reconciled);
    let mut f = restarted(f, &after, &reopen);
    f.reject(&w, &e);
    assert_eq!(run(f.db.rollback(&e)), Ok(()));
    let (f, last) = f.snapshot();
    current(&last, &w, corrected);
    f.finish()
}

/// EO4 (win): two pending writers on the corrected field both win, chained exactly.
pub fn eo4_win<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    let e1 = f.patch(20, &w, &[("body", Some("E1"))]);
    let e2 = f.patch(30, &w, &[("body", Some("E2"))]);
    f.push(31);
    corrected(&mut f, &w, &a);
    let (mut f, before) = f.snapshot();
    let first = img(&w, &[("body", Some("E1"))], &[("body", 20, false)]);
    let second = img(&w, &[("body", Some("E2"))], &[("body", 30, false)]);
    let optimistic = img(&w, &[("body", Some("A"))], &[("body", 10, false)]);
    exact(&before, &e1, &w, optimistic, first.clone());
    exact(&before, &e2, &w, first.clone(), second.clone());
    current(&before, &w, second.clone());
    assert_eq!(
        run(f.db.reconcile(&a, typed(&w, &[("body", "S", 15)]))),
        Ok(())
    );
    let (f, after) = f.snapshot();
    let corrected = img(&w, &[("body", Some("S"))], &[("body", 15, true)]);
    exact(&after, &e1, &w, corrected, first.clone());
    exact(&after, &e2, &w, first, second.clone());
    current(&after, &w, second);
    preserved(&before, &after, &e1);
    preserved(&before, &after, &e2);
    others_identical(&before, &after, &[&a, &e1, &e2]);
    f.finish()
}

/// EO5: rollback of a rejected P on body and extra with a pending E on extra. Body returns to
/// P.before in both of E's images and in the current row; extra keeps E's value.
pub fn eo5<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig")), ("extra", Some("x0"))]);
    let p = f.patch(10, &w, &[("body", Some("p")), ("extra", Some("pg"))]);
    let e = f.patch(20, &w, &[("extra", Some("e"))]);
    f.push(21);
    f.reject(&w, &p);
    let (mut f, before) = f.snapshot();
    exact(
        &before,
        &e,
        &w,
        img(
            &w,
            &[("body", Some("p")), ("extra", Some("pg"))],
            &[("body", 10, false), ("extra", 10, false)],
        ),
        img(
            &w,
            &[("body", Some("p")), ("extra", Some("e"))],
            &[("body", 10, false), ("extra", 20, false)],
        ),
    );
    assert_eq!(run(f.db.rollback(&p)), Ok(()));
    let (f, after) = f.snapshot();
    let pending = img(
        &w,
        &[("body", Some("orig")), ("extra", Some("e"))],
        &[("body", 5, true), ("extra", 20, false)],
    );
    let restored = img(
        &w,
        &[("body", Some("orig")), ("extra", Some("x0"))],
        &[("body", 5, true), ("extra", 5, true)],
    );
    exact(&after, &e, &w, restored, pending.clone());
    current(&after, &w, pending);
    preserved(&before, &after, &e);
    others_identical(&before, &after, &[&p, &e]);
    assert!(after.ledger.records[&p].rolled_back);
    f.finish()
}

/// EO16: a finalized E that changed only `extra` is preserved, on the correction and the rollback
/// path (L is empty).
pub fn eo16<S: SyncStorage>(storage: S, rollback: bool) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig")), ("extra", Some("x0"))]);
    let k = f.patch(10, &w, &[("body", Some("K"))]);
    let e = f.patch(20, &w, &[("extra", Some("e"))]);
    f.push(21);
    if rollback {
        f.reject(&w, &k);
    }
    f.confirm(&w, &e);
    let (mut f, before) = f.snapshot();
    current(
        &before,
        &w,
        img(
            &w,
            &[("body", Some("K")), ("extra", Some("e"))],
            &[("body", 10, false), ("extra", 20, true)],
        ),
    );
    if rollback {
        assert_eq!(run(f.db.rollback(&k)), Ok(()));
    } else {
        corrected(&mut f, &w, &k);
        assert_eq!(
            run(f.db.reconcile(&k, typed(&w, &[("body", "S", 15)]))),
            Ok(())
        );
    }
    let (f, after) = f.snapshot();
    let (body, body_stamp) = if rollback {
        ("orig", 5)
    } else {
        ("S", 15)
    };
    current(
        &after,
        &w,
        img(
            &w,
            &[("body", Some(body)), ("extra", Some("e"))],
            &[("body", body_stamp, true), ("extra", 20, true)],
        ),
    );
    let record = &after.ledger.records[&k];
    others_identical(&before, &after, &[&k]);
    assert!(if rollback {
        record.rolled_back
    } else {
        record.reconciled
    });
    f.finish()
}

/// EO17: removal, explicit Null and a missing stamp stay distinct through C4 and a later rollback.
/// `rollback` selects P (rejected) instead of A (Corrected); `removal_first` selects K removes
/// extra and L sets Null, otherwise K sets Null and L removes. K runs at clock 0 (a present
/// timestamp 0); `memo` is never written, so no image ever has its value or a stamp. The store
/// restarts (a fresh connection on SQLite) before C4, after C4 and before L's rollback, and at
/// the end through the runner.
pub fn eo17<S: SyncStorage>(
    storage: S,
    rollback: bool,
    removal_first: bool,
    reopen: impl Fn(S) -> S,
) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig")), ("extra", Some("x0"))]);
    let kv = if rollback {
        "p"
    } else {
        "A"
    };
    let k = if removal_first {
        f.replace(0, &w, &[("body", Some(kv))])
    } else {
        f.patch(0, &w, &[("extra", None)])
    };
    let l = if removal_first {
        f.patch(7, &w, &[("extra", None)])
    } else {
        f.replace(7, &w, &[("body", Some("orig"))])
    };
    f.push(8);
    let (f, before) = f.snapshot();
    assert_eq!(
        before.ledger.records[if removal_first {
            &k
        } else {
            &l
        }]
        .entity
        .as_ref()
        .unwrap()
        .removed_fields,
        BTreeSet::from(["extra".to_string()]),
        "setup: replace declares the removal"
    );
    let (l_before0, l_after0) = if removal_first {
        (
            img(
                &w,
                &[("body", Some(kv))],
                &[("body", 0, false), ("extra", 0, false)],
            ),
            img(
                &w,
                &[("body", Some(kv)), ("extra", None)],
                &[("body", 0, false), ("extra", 7, false)],
            ),
        )
    } else {
        (
            img(
                &w,
                &[("body", Some("orig")), ("extra", None)],
                &[("body", 5, true), ("extra", 0, false)],
            ),
            img(
                &w,
                &[("body", Some("orig"))],
                &[("body", 7, false), ("extra", 7, false)],
            ),
        )
    };
    exact(&before, &l, &w, l_before0, l_after0.clone());
    current(&before, &w, l_after0);
    // A restart before C4 too, so the reopen step is exercised on unchanged code.
    let mut f = restarted(f, &before, &reopen);
    if rollback {
        f.reject(&w, &k);
        assert_eq!(run(f.db.rollback(&k)), Ok(()));
    } else {
        corrected(&mut f, &w, &k);
        assert_eq!(
            run(f.db.reconcile(&k, typed(&w, &[("body", "S", 3)]))),
            Ok(())
        );
    }
    let (f, after) = f.snapshot();
    // eff0: the rollback restores P.before (present x0 at 5); the correction keeps K's own state
    // of extra (absent or Null) with K's timestamp 0 promoted by C1a, and body S at 3.
    let eff0 = match (rollback, removal_first) {
        (true, _) => img(
            &w,
            &[("body", Some("orig")), ("extra", Some("x0"))],
            &[("body", 5, true), ("extra", 5, true)],
        ),
        (false, true) => img(
            &w,
            &[("body", Some("S"))],
            &[("body", 3, true), ("extra", 0, true)],
        ),
        (false, false) => img(
            &w,
            &[("body", Some("S")), ("extra", None)],
            &[("body", 3, true), ("extra", 0, true)],
        ),
    };
    // L.after: L's declared state and stamp; a removal is absent, an explicit Null is present.
    let l_after = match (rollback, removal_first) {
        (true, true) => img(
            &w,
            &[("body", Some("orig")), ("extra", None)],
            &[("body", 5, true), ("extra", 7, false)],
        ),
        (false, true) => img(
            &w,
            &[("body", Some("S")), ("extra", None)],
            &[("body", 3, true), ("extra", 7, false)],
        ),
        (_, false) => img(
            &w,
            &[("body", Some("orig"))],
            &[("body", 7, false), ("extra", 7, false)],
        ),
    };
    exact(&after, &l, &w, eff0.clone(), l_after.clone());
    current(&after, &w, l_after);
    preserved(&before, &after, &l);
    others_identical(&before, &after, &[&k, &l]);
    let record = &after.ledger.records[&k];
    assert!(if rollback {
        record.rolled_back
    } else {
        record.reconciled
    });
    let mut f = restarted(f, &after, &reopen);
    f.reject(&w, &l);
    assert_eq!(run(f.db.rollback(&l)), Ok(()));
    let (f, last) = f.snapshot();
    current(&last, &w, eff0);
    f.finish()
}

// ---------------------------------------------------------------- characterization (refusals)

/// G2 boundary (characterization): pending L declares the never-stamped `memo` at clock 0. A
/// missing stamp counts as time 0 for the comparison only, so L does not beat eff0 and the action
/// refuses, although E6 holds for the owned body.
pub fn g2_missing_stamp_boundary<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    let l = f.patch(0, &w, &[("memo", Some("m"))]);
    f.push(11);
    corrected(&mut f, &w, &a);
    let (f, state) = f.snapshot();
    // B (= L.before = A.after) has no memo value and no memo stamp; L.after has memo {0,false}.
    let base = img(&w, &[("body", Some("A"))], &[("body", 10, false)]);
    let original = img(&w, &[("body", Some("orig"))], &[("body", 5, true)]);
    exact(&state, &a, &w, original, base.clone());
    let declared = img(
        &w,
        &[("body", Some("A")), ("memo", Some("m"))],
        &[("body", 10, false), ("memo", 0, false)],
    );
    exact(&state, &l, &w, base, declared.clone());
    current(&state, &w, declared);
    let plan = typed(&w, &[("body", "S", 15)]);
    refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    })
    .finish()
}

/// EO3 and the equal-stamp refusal: a correction at or after the pending writer's clock refuses
/// (G2), changing nothing.
pub fn eo3<S: SyncStorage>(storage: S, correction_time: i64) -> Outcome<S> {
    let (f, w, a, _) = correction_over_pending(storage, 10, 20);
    let plan = typed(&w, &[("body", "S", correction_time)]);
    refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    })
    .finish()
}

/// EO4 (refuse): with the correction at 25, the first pending writer (20) does not win.
pub fn eo4_refuse<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    f.patch(20, &w, &[("body", Some("E1"))]);
    f.patch(30, &w, &[("body", Some("E2"))]);
    f.push(31);
    corrected(&mut f, &w, &a);
    let plan = typed(&w, &[("body", "S", 25)]);
    refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    })
    .finish()
}

/// EO15 (public): a correction field A did not declare was changed by a finalized write (E6).
pub fn eo15<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig")), ("extra", Some("x0"))]);
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    let e = f.patch(20, &w, &[("extra", Some("e"))]);
    f.push(21);
    f.confirm(&w, &e);
    corrected(&mut f, &w, &a);
    let plan = typed(&w, &[("body", "S", 15), ("extra", "X", 15)]);
    refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    })
    .finish()
}

/// EO14 (public): a pull changing an untouched field cannot interleave while A or L is live.
pub fn eo14_pull<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    f.patch(20, &w, &[("extra", Some("l"))]);
    f.push(21);
    let change = EntityChange {
        table: "notes".into(),
        record_id: w.clone(),
        op: EntityOperation::Update,
        changes: fields(&[("memo", Some("pulled"))]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 30,
        field_timestamps: BTreeMap::from([("memo".into(), 30)]),
    };
    refused(f, &a, CommandError::PullConflict, |db| {
        let request = db.begin_pull().unwrap().request;
        let page = PullPage {
            checkpoint: "c1".into(),
            complete: true,
            changes: vec![change],
        };
        let result = run(db.apply_pull(request, page));
        db.abort_pull(request).unwrap();
        result.map(|_| ())
    })
    .finish()
}

/// EO-PRECEDENCE: earlier errors keep their precedence, an outside-closure write keeps the
/// SPEC-0039 refusal, and C1 keeps its success path.
pub fn eo_precedence<S: SyncStorage>(storage: S) -> Outcome<S> {
    let (mut f, w, a, e) = correction_over_pending(storage, 10, 20);
    let other = f.row(6, &[("body", Some("other"))]);
    let plan = typed(&w, &[("body", "S", 15)]);
    let guard = |id: &str, expected: Option<BTreeMap<String, Value>>| CorrectionGuard {
        table: "notes".into(),
        id: id.into(),
        expected,
    };
    let guards = [
        guard(&w, None),     // a guard on W is unused (W is in the closure)
        guard("nope", None), // an unused guard
        guard("", None),     // malformed: empty identity
        guard(
            // malformed: the expected row violates the schema
            &other,
            Some(BTreeMap::from([("body".into(), Value::Integer(1))])),
        ),
    ];
    for guard in guards {
        let plan = plan.clone();
        f = refused(f, &a, CommandError::InvalidCommand, |db| {
            run(db.reconcile_guarded(&a, plan, vec![guard]))
        });
    }
    // An Update record creates nothing, so any identity mapping is a plan-shape error.
    let mut mapped = plan.clone();
    mapped.mappings = BTreeMap::from([(w.clone(), w.clone())]);
    f = refused(f, &a, CommandError::InvalidCommand, |db| {
        run(db.reconcile(&a, mapped))
    });
    let outside = typed(&other, &[("body", "S", 15)]);
    f = refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, outside))
    });
    let (mut f, before) = f.snapshot();
    let pending = img(&w, &[("body", Some("E"))], &[("body", 20, false)]);
    let optimistic = img(&w, &[("body", Some("A"))], &[("body", 10, false)]);
    exact(&before, &e, &w, optimistic, pending.clone());
    assert_eq!(
        run(f.db.reconcile(&a, ReconcilePlan::default())),
        Ok(()),
        "C1 unchanged"
    );
    let (f, after) = f.snapshot();
    // C1 only confirms: E's row images, E.after and the current cell are unchanged; C1a promotes
    // E.before, which holds A's accepted value and timestamp. Nothing is re-based (no C4).
    let promoted = img(&w, &[("body", Some("A"))], &[("body", 10, true)]);
    exact(&after, &e, &w, promoted, pending.clone());
    current(&after, &w, pending);
    preserved(&before, &after, &e);
    f.finish()
}

/// EO-EXIT: a refused correction exits only through C1; a processed rejection is not an exit.
pub fn eo_exit<S: SyncStorage>(storage: S) -> Outcome<S> {
    let (f, w, a, _) = correction_over_pending(storage, 10, 20);
    let plan = typed(&w, &[("body", "S", 25)]);
    let mut f = refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    });
    assert_eq!(
        run(f.db.reconcile(&a, ReconcilePlan::default())),
        Ok(()),
        "C1 exit"
    );
    let p = f.patch(30, &w, &[("body", Some("P"))]);
    f.push(31);
    f.reject(&w, &p);
    let (f, state) = f.snapshot();
    let entry = state.status.commands.iter().find(|c| c.key == p).unwrap();
    assert!(!entry.rolled_back, "processing a rejection is not an exit");
    assert!(state.ledger.records[&p].reconciled);
    f.finish()
}

/// The EO-REFUSE-LIVE variants on the correction path.
#[derive(Clone, Copy, Debug)]
pub enum Live {
    Named,
    Cancelled,
    AppliedUnreconciled,
    CorrectedUnreconciled,
    RejectedUnreconciled,
    Earlier,
    DeleteInL,
    MergeHook,
    TypedDelete,
    TypedCreate,
}

fn keep_default(input: &MergeInput<'_>) -> Result<EntityResolution, CommandError> {
    Ok(EntityResolution {
        row: input.default.row.clone(),
        metadata: input.default.metadata.clone(),
    })
}

/// A typed correction of W with the given operation (Create and Delete are ineligible shapes).
fn typed_op(id: &str, op: EntityOperation) -> ReconcilePlan {
    let changes = if op == EntityOperation::Delete {
        BTreeMap::new()
    } else {
        fields(&[("body", Some("S"))])
    };
    ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Entity {
            change: EntityChange {
                table: "notes".into(),
                record_id: id.into(),
                op,
                field_timestamps: changes.keys().map(|k| (k.clone(), 15)).collect(),
                changes,
                removed_fields: BTreeSet::new(),
                timestamp_ms: 15,
            },
        }],
    }
}

pub fn refuse_live<S: SyncStorage>(storage: S, case: Live) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    if matches!(case, Live::MergeHook) {
        f.hook();
    }
    let w = f.row(5, &[("body", Some("orig"))]);
    if matches!(case, Live::Earlier) {
        f.patch(8, &w, &[("body", Some("earlier"))]);
    }
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    let later = match case {
        Live::Named => edit(&mut f.db, &w, "named", 20),
        Live::DeleteInL => {
            f.at(20);
            assert!(run(f.db.remove("notes", &w)).unwrap());
            f.last()
        }
        _ => f.patch(20, &w, &[("body", Some("E"))]),
    };
    if matches!(case, Live::Cancelled) {
        f.patch(30, &w, &[("body", Some("F"))]);
    }
    f.push(31);
    match case {
        Live::Cancelled => {
            assert_eq!(
                run(f.db.cancel(&later)),
                Err(CommandError::RollbackConflict)
            );
            assert_eq!(
                f.db.queue_status()
                    .commands
                    .iter()
                    .find(|c| c.key == later)
                    .unwrap()
                    .state,
                SyncState::Cancelled,
                "setup: live cancelled"
            );
        }
        Live::AppliedUnreconciled => f.outcome(&w, &later, RemoteOutcome::Applied(Json::Null)),
        Live::CorrectedUnreconciled => f.outcome(&w, &later, RemoteOutcome::Corrected(Json::Null)),
        Live::RejectedUnreconciled => f.outcome(&w, &later, RemoteOutcome::Rejected("no".into())),
        _ => {}
    }
    corrected(&mut f, &w, &a);
    let plan = match case {
        Live::TypedDelete => typed_op(&w, EntityOperation::Delete),
        Live::TypedCreate => typed_op(&w, EntityOperation::Create),
        _ => typed(&w, &[("body", "S", 15)]),
    };
    refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    })
    .finish()
}

/// EO-REFUSE-LIVE on the rollback path: a live named command, earlier live work, an Applied
/// unreconciled later record, a delete in L, a merge hook, and a later stamp equal to or older
/// than P.before's (G2).
#[derive(Clone, Copy, Debug)]
pub enum RollbackLive {
    Named,
    Earlier,
    AppliedUnreconciled,
    DeleteInL,
    MergeHook,
    EqualStamp,
    OlderStamp,
}

pub fn refuse_rollback<S: SyncStorage>(storage: S, case: RollbackLive) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    if matches!(case, RollbackLive::MergeHook) {
        f.hook();
    }
    let w = f.row(5, &[("body", Some("orig"))]);
    if matches!(case, RollbackLive::Earlier) {
        f.patch(8, &w, &[("body", Some("earlier"))]);
    }
    let p = f.patch(10, &w, &[("body", Some("p"))]);
    let later = match case {
        RollbackLive::Named => edit(&mut f.db, &w, "named", 20),
        RollbackLive::DeleteInL => {
            f.at(20);
            assert!(run(f.db.remove("notes", &w)).unwrap());
            f.last()
        }
        RollbackLive::EqualStamp => f.patch(5, &w, &[("body", Some("e"))]),
        RollbackLive::OlderStamp => f.patch(4, &w, &[("body", Some("e"))]),
        _ => f.patch(20, &w, &[("body", Some("e"))]),
    };
    f.push(31);
    f.reject(&w, &p);
    if matches!(case, RollbackLive::AppliedUnreconciled) {
        f.outcome(&w, &later, RemoteOutcome::Applied(Json::Null));
    }
    if let RollbackLive::EqualStamp | RollbackLive::OlderStamp = case {
        // Setup: P.before(body) is stamped 5; L's own stamp does not beat it.
        let time = if matches!(case, RollbackLive::EqualStamp) {
            5
        } else {
            4
        };
        let (next, state) = f.snapshot();
        f = next;
        exact(
            &state,
            &p,
            &w,
            img(&w, &[("body", Some("orig"))], &[("body", 5, true)]),
            img(&w, &[("body", Some("p"))], &[("body", 10, false)]),
        );
        exact(
            &state,
            &later,
            &w,
            img(&w, &[("body", Some("p"))], &[("body", 10, false)]),
            img(&w, &[("body", Some("e"))], &[("body", time, false)]),
        );
    }
    refused(f, &p, CommandError::RollbackConflict, |db| {
        run(db.rollback(&p))
    })
    .finish()
}

/// EO-CANCEL (characterization of API-0006's cancel-conflict transition): cancel of a processed
/// rejection over overlapping later work returns RollbackConflict and never uses C4. Rows,
/// metadata and L are unchanged; P is durably Cancelled and reconciled, not rolled back, and stays
/// live with its recovery data. The status update is expected; no data or outcome notification.
pub fn cancel_processed_rejection<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let p = f.patch(10, &w, &[("body", Some("p"))]);
    let e = f.patch(20, &w, &[("body", Some("e"))]);
    f.push(21);
    f.reject(&w, &p);
    let (mut f, before) = f.snapshot();
    let query = f.db.query("notes");
    let data = run(f.db.observe(&query)).unwrap();
    let outcomes = [
        f.db.observe_outcome(&p).unwrap(),
        f.db.observe_outcome(&e).unwrap(),
    ];
    let status = f.db.observe_queue_status().unwrap();
    f.db.drain_events(0);
    assert_eq!(run(f.db.cancel(&p)), Err(CommandError::RollbackConflict));
    let events = f.db.drain_events(0);
    assert!(
        !events.iter().any(|ev| matches!(ev, CommandEvent::Data(n) if n.subscription == data)
            || matches!(ev, CommandEvent::Outcome { subscription, .. } if outcomes.contains(subscription))),
        "no data or outcome notification"
    );
    let statuses = events
        .iter()
        .filter_map(|ev| match ev {
            CommandEvent::Status {
                subscription,
                value,
            } if *subscription == status => Some(value.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let (f, after) = f.snapshot();
    assert!(
        statuses.len() == 1 && statuses[0] == after.status,
        "exactly the expected status update"
    );
    let entry = after.status.commands.iter().find(|c| c.key == p);
    let entry = entry.expect("P stays live");
    assert_eq!(
        (entry.state.clone(), entry.rolled_back),
        (SyncState::Cancelled, false)
    );
    assert_eq!(after.notes, before.notes, "no row change");
    assert_eq!(after.current(&w), before.current(&w), "no metadata change");
    assert!(
        after.ledger.records[&e] == before.ledger.records[&e],
        "L unchanged, never re-based"
    );
    let prior = &before.ledger.records[&p];
    assert!(prior.state == SyncState::Rejected && prior.reconciled && !prior.rolled_back);
    assert!(!prior.undo.is_empty() && !prior.metadata_undo.is_empty());
    let mut expected = prior.clone();
    expected.state = SyncState::Cancelled;
    assert!(
        after.ledger.records[&p] == expected,
        "P: only the state changes to Cancelled; reconciled, recovery data kept"
    );
    assert!(after.pending == before.pending);
    f.finish()
}

/// EO-REFUSE-OWNED: a non-live record that changed an owned field refuses through E6.
#[derive(Clone, Copy, Debug)]
pub enum Owned {
    FinalizedEntity,
    Trimmed,
    LegacyNamed,
}

pub fn refuse_owned<S: SyncStorage>(storage: S, case: Owned) -> Outcome<S> {
    let mut f = if matches!(case, Owned::Trimmed) {
        Fixture::with_history(storage, 0)
    } else {
        Fixture::new(storage)
    };
    let w = f.row(5, &[("body", Some("orig"))]);
    let a = f.patch(10, &w, &[("body", Some("A"))]);
    match case {
        Owned::FinalizedEntity | Owned::Trimmed => {
            let e = f.patch(20, &w, &[("body", Some("E"))]);
            f.push(21);
            f.confirm(&w, &e);
            if matches!(case, Owned::Trimmed) {
                let (next, state) = f.snapshot();
                f = next;
                assert!(!state.ledger.records.contains_key(&e), "setup: E trimmed");
            }
        }
        Owned::LegacyNamed => {
            // RD withholds a later named command while A is unresolved; only a pre-RD (legacy)
            // Sent pair can confirm it first (audit finding A2, EO10b).
            let c = edit(&mut f.db, &w, "named", 20);
            f = f.with_storage(force_sent);
            f.db.receive(&c, Some(RemoteOutcome::Applied(Json::Null)))
                .unwrap();
            run(f.db.reconcile(&c, ReconcilePlan::default())).unwrap();
            // Non-vacuity (EO10b): W's stamp still equals A.after's while its value differs, so
            // a stamp alone would wrongly prove A's state.
            let (next, state) = f.snapshot();
            f = next;
            let record = &state.ledger.records[&a];
            let a_after = record.undo[0].after.as_ref().unwrap();
            let a_meta = record.metadata_undo[0].after.as_ref().unwrap();
            assert_eq!(state.current(&w).fields["body"], a_meta.fields["body"]);
            assert_ne!(
                state.row(&w).unwrap().fields["body"],
                a_after.fields["body"]
            );
        }
    }
    corrected(&mut f, &w, &a);
    let plan = typed(&w, &[("body", "S", 15)]);
    refused(f, &a, CommandError::RollbackConflict, |db| {
        run(db.reconcile(&a, plan))
    })
    .finish()
}

/// G1-OPEN: a processed rejected P whose owned field a finalized write changed stays stuck.
/// Rollback refuses with nothing changed; cancel returns RollbackConflict but makes P Cancelled
/// and reconciled, still live and not rolled back, with rows and metadata unchanged; forget
/// returns Protocol. G1 is not claimed as fixed or as limited to this trace.
pub fn g1_open<S: SyncStorage>(storage: S) -> Outcome<S> {
    let mut f = Fixture::new(storage);
    let w = f.row(5, &[("body", Some("orig"))]);
    let p = f.patch(10, &w, &[("body", Some("p"))]);
    let e = f.patch(20, &w, &[("body", Some("e"))]);
    f.push(21);
    f.reject(&w, &p);
    f.confirm(&w, &e);
    let f = refused(f, &p, CommandError::RollbackConflict, |db| {
        run(db.rollback(&p))
    });
    let (mut f, before) = f.snapshot();
    assert_eq!(run(f.db.cancel(&p)), Err(CommandError::RollbackConflict));
    assert_eq!(f.db.forget(&p), Err(CommandError::Protocol));
    let (f, after) = f.snapshot();
    let entry = after.status.commands.iter().find(|c| c.key == p);
    let entry = entry.expect("P stays live");
    assert_eq!(
        (entry.state.clone(), entry.rolled_back),
        (SyncState::Cancelled, false)
    );
    assert!(after.ledger.records[&p].reconciled);
    assert_eq!(after.notes, before.notes, "no row change");
    assert_eq!(after.current(&w), before.current(&w), "no metadata change");
    current(
        &after,
        &w,
        img(&w, &[("body", Some("e"))], &[("body", 20, true)]),
    );
    f.finish()
}
