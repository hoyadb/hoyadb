//! SPEC-0035 / TC-0039: corrected clock samples recover retries without changing identity.
use hoyadb_core::model::*;
use hoyadb_core::{command::*, storage::*};
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("immediate adapter suspended"),
    }
}

/// initial 1000, multiplier 2, max 4000: the largest jittered delay is C = 4000 + 1000.
pub const C: i64 = 5000;
pub const R: i64 = 1_000_000;
pub const F: i64 = R + 10 * C;

pub fn options() -> SyncOptions {
    SyncOptions {
        initial_delay_ms: 1000,
        multiplier: 2,
        max_delay_ms: 4000,
        ..SyncOptions::default()
    }
}

pub fn open<S: SyncStorage>(storage: S) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [39; 16],
        options(),
    ))
    .unwrap()
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "parents".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "name".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

fn definition() -> CommandDefinition {
    CommandDefinition {
        name: "create-parent-v1".into(),
        arguments: BTreeMap::from([(
            "label".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        reference_paths: vec![],
    }
}

pub fn enqueue<S: SyncStorage>(db: &mut CommandDatabase<S>, label: &str, now_ms: i64) -> String {
    let r = db.request().unwrap();
    let args = Json::Object(BTreeMap::from([(
        "label".into(),
        Json::String(label.into()),
    )]));
    assert!(run(db.begin(r, definition(), args, Some(Json::Null), now_ms)).unwrap());
    run(db.scope_insert(
        r,
        "parents",
        BTreeMap::from([("name".into(), Value::String(label.into()))]),
    ))
    .unwrap();
    run(db.finish(r, Json::Null)).unwrap().key
}

pub fn entry<S: SyncStorage>(db: &CommandDatabase<S>, key: &str) -> QueueEntry {
    db.queue_status()
        .commands
        .into_iter()
        .find(|e| e.key == key)
        .expect("queued key")
}

/// Sends one command at the forward sample F and returns its key and frozen envelope.
pub fn sent_at_forward_jump<S: SyncStorage>(db: &mut CommandDatabase<S>) -> (String, String) {
    let key = enqueue(db, "forward", R);
    let batch = db.prepare_push(F, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 1);
    assert_eq!(batch.next_due_ms, Some(F + 1000));
    (key, batch.envelopes[0].clone())
}

/// CS01, CS02 and CS08 through the public owner.
pub fn corrected_jump<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, envelope) = sent_at_forward_jump(&mut db);
    // CS01: a corrected sample emits nothing early but bounds the retained deadline.
    let corrected = db.prepare_push(R, 0).unwrap();
    assert!(corrected.envelopes.is_empty(), "no premature envelope");
    let due = corrected.next_due_ms.expect("Sent deadline");
    assert!(
        due <= R + C,
        "deadline {due} must be within raw sample plus maximum jittered delay"
    );
    let capped = entry(&db, &key);
    assert_eq!(
        (capped.state, capped.attempts, capped.next_due_ms),
        (SyncState::Sent, 1, Some(R + C))
    );
    // CS02: progressing samples before the cap neither send nor extend it.
    for sample in [R + 1, R + C / 2, R + C - 1] {
        let batch = db.prepare_push(sample, 0).unwrap();
        assert!(batch.envelopes.is_empty());
        assert_eq!(batch.next_due_ms, Some(R + C), "no extension at {sample}");
    }
    let retry = db.prepare_push(R + C, 0).unwrap();
    assert_eq!(
        retry.envelopes,
        vec![envelope],
        "exact original frozen bytes"
    );
    assert_eq!(
        entry(&db, &key).attempts,
        2,
        "attempt advances exactly once"
    );
    // CS08: a new command after correction schedules from the raw sample while
    // the identity floor stays at F, so keys remain unique and ordered.
    let later = enqueue(&mut db, "after correction", R + C + 10);
    assert!(later > key, "identity floor keeps keys monotonic");
    let batch = db.prepare_push(R + C + 10, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 1);
    assert_eq!(entry(&db, &later).next_due_ms, Some(R + C + 10 + 1000));
    db.close();
}

/// CS09 through the public owner: a backward sample does not treat the identity floor as elapsed time.
pub fn backward_sample<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let key = enqueue(&mut db, "backward", 100_000);
    let first = db.prepare_push(100_000, 0).unwrap();
    assert_eq!(first.next_due_ms, Some(101_000));
    // A later identity floor without making the record due: an unrelated command at 101_500.
    enqueue(&mut db, "floor", 101_500);
    let backward = db.prepare_push(100_500, 0).unwrap();
    assert!(
        !backward.envelopes.iter().any(|e| e == &first.envelopes[0]),
        "deadline 101000 is not reached by raw sample 100500"
    );
    assert_eq!(entry(&db, &key).next_due_ms, Some(101_000));
    let due = db.prepare_push(101_000, 0).unwrap();
    assert!(due.envelopes.contains(&first.envelopes[0]));
    db.close();
}
