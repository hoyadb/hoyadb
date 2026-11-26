//! SPEC-0037 R4: per-thread physical work counters, compiled only with the
//! non-default `work-counters` feature. Each counter counts entries, not calls.
use std::cell::Cell;
use std::collections::BTreeSet;

/// TC-0041's test-only observation of the last ledger transaction's captured
/// identities, plus the owner's current committed epoch and tombstone cache.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerWorkState {
    pub record_keys: BTreeSet<String>,
    pub metadata_keys: BTreeSet<(String, String)>,
    pub epoch: u64,
    pub cached_tombstones: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkSnapshot {
    pub rows_cloned: u64,
    pub ledger_entries_cloned: u64,
    pub metadata_entries_visited: u64,
    pub packets_encoded: u64,
    pub packets_parsed: u64,
    pub rows_validated: u64,
    pub inventory_rows_validated: u64,
    pub memory_rows_visited: u64,
    pub ordered_ops: u64,
    pub sqlite_rows_read: u64,
    pub sqlite_rows_written: u64,
    pub sqlite_fullscan_steps: u64,
    pub sqlite_sort: u64,
    pub sqlite_vm_steps: u64,
}

#[derive(Default)]
pub struct WorkCounters {
    pub rows_cloned: Cell<u64>,
    pub ledger_entries_cloned: Cell<u64>,
    pub metadata_entries_visited: Cell<u64>,
    pub packets_encoded: Cell<u64>,
    pub packets_parsed: Cell<u64>,
    pub rows_validated: Cell<u64>,
    pub inventory_rows_validated: Cell<u64>,
    pub memory_rows_visited: Cell<u64>,
    pub ordered_ops: Cell<u64>,
    pub sqlite_rows_read: Cell<u64>,
    pub sqlite_rows_written: Cell<u64>,
    pub sqlite_fullscan_steps: Cell<u64>,
    pub sqlite_sort: Cell<u64>,
    pub sqlite_vm_steps: Cell<u64>,
}

thread_local! {
    static COUNTERS: WorkCounters = WorkCounters::default();
}

/// Adds `entries` to one counter of the current thread.
pub fn add(counter: fn(&WorkCounters) -> &Cell<u64>, entries: u64) {
    COUNTERS.with(|counters| {
        let cell = counter(counters);
        cell.set(cell.get().saturating_add(entries));
    });
}

pub fn reset() {
    COUNTERS.with(|counters| {
        for cell in [
            &counters.rows_cloned,
            &counters.ledger_entries_cloned,
            &counters.metadata_entries_visited,
            &counters.packets_encoded,
            &counters.packets_parsed,
            &counters.rows_validated,
            &counters.inventory_rows_validated,
            &counters.memory_rows_visited,
            &counters.ordered_ops,
            &counters.sqlite_rows_read,
            &counters.sqlite_rows_written,
            &counters.sqlite_fullscan_steps,
            &counters.sqlite_sort,
            &counters.sqlite_vm_steps,
        ] {
            cell.set(0);
        }
    });
}

pub fn snapshot() -> WorkSnapshot {
    COUNTERS.with(|counters| WorkSnapshot {
        rows_cloned: counters.rows_cloned.get(),
        ledger_entries_cloned: counters.ledger_entries_cloned.get(),
        metadata_entries_visited: counters.metadata_entries_visited.get(),
        packets_encoded: counters.packets_encoded.get(),
        packets_parsed: counters.packets_parsed.get(),
        rows_validated: counters.rows_validated.get(),
        inventory_rows_validated: counters.inventory_rows_validated.get(),
        memory_rows_visited: counters.memory_rows_visited.get(),
        ordered_ops: counters.ordered_ops.get(),
        sqlite_rows_read: counters.sqlite_rows_read.get(),
        sqlite_rows_written: counters.sqlite_rows_written.get(),
        sqlite_fullscan_steps: counters.sqlite_fullscan_steps.get(),
        sqlite_sort: counters.sqlite_sort.get(),
        sqlite_vm_steps: counters.sqlite_vm_steps.get(),
    })
}
