//! TC-0018 N07–N09: core owns membership and staged execution semantics.
use hoyadb_cross_platform_spike::{Notification, Order, Store};

fn row(id: &str, status: &str) -> Order {
    Order {
        id: id.into(),
        status: status.into(),
        total_cents: 1,
        paid: false,
    }
}

#[test]
fn initial_matching_and_irrelevant_events() {
    let mut store = Store::default();
    store.insert(row("a", "pending"));
    let token = store.observe("pending").unwrap();
    assert!(store.is_observing(token));
    assert_eq!(
        store.take_notifications(),
        vec![Notification {
            subscription: token,
            count: 1
        }]
    );
    store.insert(row("b", "complete"));
    assert!(store.take_notifications().is_empty());
    store.insert(row("c", "pending"));
    assert_eq!(
        store.take_notifications(),
        vec![Notification {
            subscription: token,
            count: 2
        }]
    );
}

#[test]
fn unsubscribe_removes_pending_and_later_events() {
    let mut store = Store::default();
    let token = store.observe("pending").unwrap();
    assert!(store.unobserve(token));
    assert!(!store.unobserve(token));
    assert!(!store.is_observing(token));
    store.insert(row("a", "pending"));
    assert!(store.take_notifications().is_empty());
    assert_ne!(store.observe("pending").unwrap(), token);
}

#[test]
fn drain_is_a_snapshot_and_does_not_consume_future_events() {
    let mut store = Store::default();
    let token = store.observe("pending").unwrap();
    let old = store.take_notifications();
    store.insert(row("a", "pending"));
    assert_eq!(
        old,
        vec![Notification {
            subscription: token,
            count: 0
        }]
    );
    assert_eq!(
        store.take_notifications(),
        vec![Notification {
            subscription: token,
            count: 1
        }]
    );
    assert!(store.take_notifications().is_empty());
}

#[test]
fn staged_reads_hide_writes_until_one_atomic_finish_event() {
    let mut store = Store::default();
    let subscription = store.observe("pending").unwrap();
    store.take_notifications();
    let scope = store.begin_scope().unwrap();
    assert!(store.begin_scope().is_none());
    let a = row("a", "pending");
    let b = row("b", "pending");
    assert!(store.scope_insert(scope, a.clone()));
    assert!(store.scope_insert(scope, b.clone()));
    assert_eq!(
        store.scope_query(scope, "pending"),
        Some(vec![a.clone(), b.clone()])
    );
    assert!(store.query_status("pending").is_empty());
    assert!(store.take_notifications().is_empty());
    assert!(store.finish_scope(scope, true));
    assert_eq!(store.query_status("pending"), vec![a, b]);
    assert_eq!(
        store.take_notifications(),
        vec![Notification {
            subscription,
            count: 2
        }]
    );
    assert!(store.scope_query(scope, "pending").is_none());
    assert!(!store.scope_insert(scope, row("c", "pending")));
    assert!(!store.finish_scope(scope, true));
    assert!(!store.finish_scope(scope, false));
    assert_ne!(store.begin_scope().unwrap(), scope);
}

#[test]
fn abort_discards_rows_and_releases_serialization_slot() {
    let mut store = Store::default();
    let sub = store.observe("pending").unwrap();
    store.take_notifications();
    store.insert(row("existing", "complete"));
    let scope = store.begin_scope().unwrap();
    assert!(store.scope_insert(scope, row("a", "pending")));
    assert_eq!(
        store.scope_query(scope, "complete"),
        Some(vec![row("existing", "complete")])
    );
    assert!(!store.finish_scope(scope + 1, true));
    assert!(store.begin_scope().is_none());
    assert!(store.finish_scope(scope, false));
    assert!(store.query_status("pending").is_empty());
    assert!(store.take_notifications().is_empty());
    assert!(store.is_observing(sub));
    assert!(!store.scope_insert(scope, row("b", "pending")));
    assert!(store.scope_query(scope, "pending").is_none());
    let next = store.begin_scope().unwrap();
    assert!(store.finish_scope(next, true));
    assert!(store.take_notifications().is_empty());
    assert!(store.begin_scope().is_some());
}
