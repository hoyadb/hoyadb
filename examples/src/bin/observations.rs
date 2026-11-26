use hoyadb_doc_examples::*;

fn main() {
    // docs:start open
    let mut db = run(Database::open(MemoryStorage::default(), schema(), vec![]))
        .unwrap()
        .into_reactive();
    // docs:end open
    // docs:start subscribe
    let query = db.query("notes");
    let rows = run(query.observe(&mut db)).unwrap();
    let count = run(query.observe_count(&mut db)).unwrap();
    // docs:end subscribe
    // docs:start initial
    let initial = db.drain_notifications(0);
    // docs:end initial
    assert_eq!(initial.len(), 2);
    // docs:start change
    run(db.insert("notes", fields("Observable"))).unwrap();
    let events = db.drain_notifications(1);
    // docs:end change
    assert!(events.iter().any(|event| event.subscription == rows));
    assert!(!events.iter().any(|event| event.subscription == count));
    // docs:start count
    let count_events = db.drain_notifications(16);
    // docs:end count
    assert_eq!(
        count_events,
        vec![Notification {
            subscription: count,
            value: Ok(ObservationValue::Count(1))
        }]
    );
    // docs:start dispose
    let removed_rows = db.unsubscribe(rows);
    let removed_count = db.unsubscribe(count);
    // docs:end dispose
    assert!(removed_rows);
    assert!(removed_count);
    assert_eq!(db.observation_stats().groups, 0);
    println!("PASS observations: initial snapshot, rows, timed count and disposal");
}
