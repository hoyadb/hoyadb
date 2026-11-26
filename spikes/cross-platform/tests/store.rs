//! SPEC-0016 scenarios S01–S05; partial REQ-0001/0003/0034 coverage only.

use hoyadb_cross_platform_spike::{Order, Store};

fn order(id: &str, status: &str, total_cents: i64, paid: bool) -> Order {
    Order {
        id: id.into(),
        status: status.into(),
        total_cents,
        paid,
    }
}

#[test]
fn s01_insert_preserves_typed_fields() {
    let mut store = Store::default();
    let row = order("order-1", "pending", 1200, false);
    store.insert(row.clone());
    assert_eq!(store.query_status("pending"), vec![row]);
}

#[test]
fn s02_equality_filters_membership() {
    let mut store = Store::default();
    let first = order("order-1", "pending", 1200, false);
    let second = order("order-2", "pending", 3500, true);
    store.insert(second.clone());
    store.insert(order("order-3", "complete", 900, true));
    store.insert(first.clone());

    let mut result = store.query_status("pending");
    result.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(result, vec![first, second]);
}

#[test]
fn s03_empty_store_returns_no_rows() {
    assert!(Store::default().query_status("pending").is_empty());
}

#[test]
fn s04_absent_value_returns_no_rows() {
    let mut store = Store::default();
    store.insert(order("order-1", "complete", 1200, true));
    assert!(store.query_status("pending").is_empty());
}

#[test]
fn s05_independent_stores_do_not_share_rows() {
    let mut first = Store::default();
    let second = Store::default();
    first.insert(order("order-1", "pending", 1200, false));
    assert!(second.query_status("pending").is_empty());
}
