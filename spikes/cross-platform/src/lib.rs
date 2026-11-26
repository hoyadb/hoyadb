//! SPEC-0016 / SPEC-0018 experiments, not the production database API.

use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub subscription: u64,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub id: String,
    pub status: String,
    pub total_cents: i64,
    pub paid: bool,
}

#[derive(Clone)]
pub struct Store {
    orders: Vec<Order>,
    next_token: u64,
    subscriptions: BTreeMap<u64, Subscription>,
    notifications: Vec<Notification>,
    scope: Option<Scope>,
}

#[derive(Clone)]
struct Subscription {
    status: String,
    ids: HashSet<String>,
}

#[derive(Clone)]
struct Scope {
    token: u64,
    staged: Vec<Order>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            orders: Vec::new(),
            next_token: 1,
            subscriptions: BTreeMap::new(),
            notifications: Vec::new(),
            scope: None,
        }
    }
}

impl Store {
    /// Committed-only owned snapshot for the isolated persistence bridge.
    pub fn committed_rows(&self) -> Vec<Order> {
        self.orders.clone()
    }

    fn token(&mut self) -> Option<u64> {
        let next = self.next_token.checked_add(1)?;
        let token = self.next_token;
        self.next_token = next;
        Some(token)
    }

    pub fn observe(&mut self, status: &str) -> Option<u64> {
        let token = self.token()?;
        let ids: HashSet<_> = self
            .query_status(status)
            .into_iter()
            .map(|row| row.id)
            .collect();
        self.notifications.push(Notification {
            subscription: token,
            count: ids.len(),
        });
        self.subscriptions.insert(
            token,
            Subscription {
                status: status.into(),
                ids,
            },
        );
        Some(token)
    }

    pub fn unobserve(&mut self, token: u64) -> bool {
        if self.subscriptions.remove(&token).is_none() {
            return false;
        }
        self.notifications
            .retain(|event| event.subscription != token);
        true
    }

    pub fn is_observing(&self, token: u64) -> bool {
        self.subscriptions.contains_key(&token)
    }

    pub fn take_notifications(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.notifications)
    }

    pub fn begin_scope(&mut self) -> Option<u64> {
        if self.scope.is_some() {
            return None;
        }
        let token = self.token()?;
        self.scope = Some(Scope {
            token,
            staged: Vec::new(),
        });
        Some(token)
    }

    pub fn scope_insert(&mut self, token: u64, row: Order) -> bool {
        let Some(scope) = self.scope.as_mut().filter(|scope| scope.token == token) else {
            return false;
        };
        scope.staged.push(row);
        true
    }

    pub fn scope_query(&self, token: u64, status: &str) -> Option<Vec<Order>> {
        let scope = self.scope.as_ref().filter(|scope| scope.token == token)?;
        let mut rows = self.query_status(status);
        rows.extend(
            scope
                .staged
                .iter()
                .filter(|row| row.status == status)
                .cloned(),
        );
        Some(rows)
    }

    pub fn finish_scope(&mut self, token: u64, commit: bool) -> bool {
        if self.scope.as_ref().map(|scope| scope.token) != Some(token) {
            return false;
        }
        let Some(scope) = self.scope.take() else {
            return false;
        };
        if commit {
            self.apply_rows(scope.staged);
        }
        true
    }

    pub fn insert(&mut self, order: Order) {
        self.apply_rows(vec![order]);
    }

    fn apply_rows(&mut self, rows: Vec<Order>) {
        let mut changed = BTreeSet::new();
        for row in rows {
            for (token, subscription) in &mut self.subscriptions {
                if row.status == subscription.status && subscription.ids.insert(row.id.clone()) {
                    changed.insert(*token);
                }
            }
            self.orders.push(row);
        }
        // One event per affected subscription, after the whole scope is visible.
        for token in changed {
            self.notifications.push(Notification {
                subscription: token,
                count: self.subscriptions[&token].ids.len(),
            });
        }
    }

    pub fn query_status(&self, status: &str) -> Vec<Order> {
        self.orders
            .iter()
            .filter(|order| order.status == status)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_tokens_do_not_wrap_or_create_subscriptions_or_scopes() {
        let mut store = Store {
            next_token: u64::MAX,
            ..Store::default()
        };
        assert!(store.observe("pending").is_none());
        assert!(store.begin_scope().is_none());
        assert!(store.subscriptions.is_empty());
        assert!(store.scope.is_none());
        assert!(store.take_notifications().is_empty());
    }
}
