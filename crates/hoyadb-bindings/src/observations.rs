//! API-0011: owned notifications and checked subscription-kind lifetimes.
use crate::{commands::status_wire, *};
use hoyadb_core::{command::*, model::*};
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Token {
    Data(Subscription),
    Sync(SyncSubscription),
}

pub(crate) fn progress_wire(value: SyncProgress) -> Json {
    object([
        (
            "phase",
            text(match value.phase {
                ProgressPhase::NotStarted => "not_started",
                ProgressPhase::Bootstrapping => "bootstrapping",
                ProgressPhase::Ready => "ready",
            }),
        ),
        ("ready", Json::Bool(value.ready)),
        ("pages", text(value.pages)),
        ("changes", text(value.changes)),
        ("tombstones", text(value.tombstones)),
    ])
}

pub(crate) fn outcome_wire(value: &CommandOutcome) -> Json {
    match value {
        CommandOutcome::Queued => object([("state", text("queued"))]),
        CommandOutcome::Sent => object([("state", text("sent"))]),
        CommandOutcome::Acknowledged => object([("state", text("acknowledged"))]),
        CommandOutcome::Applied(v) => object([("state", text("applied")), ("result", boxed(v))]),
        CommandOutcome::Corrected(v) => {
            object([("state", text("corrected")), ("result", boxed(v))])
        }
        CommandOutcome::Rejected(v) => object([("state", text("rejected")), ("reason", text(v))]),
    }
}

impl Owner {
    pub(crate) fn observation_call(&mut self, op: &str, args: &Json) -> Result<Json> {
        match op {
            "observe" => {
                let f = exact(args, &["query", "mode", "columns"])?;
                let q = query(
                    self.db.schema(),
                    &f["query"],
                    |table, source| match source {
                        Some((table, id, relation)) => self.db.related(table, id, relation),
                        None => self.db.query(table),
                    },
                )?;
                let columns = array(&f["columns"])?
                    .iter()
                    .map(|v| Ok(string(v)?.into()))
                    .collect::<Result<Vec<_>>>()?;
                let id = allocate()?;
                let mode = string(&f["mode"])?;
                if mode != "columns" && !columns.is_empty() {
                    return Err(Error("InvalidRequest"));
                }
                let sub = match mode {
                    "membership" => run(self.db.observe(&q))?,
                    "count" => run(self.db.observe_count(&q))?,
                    "columns" => run(self.db.observe_with_columns(&q, columns))?,
                    _ => return Err(Error("InvalidRequest")),
                };
                self.subscriptions.insert(id, Token::Data(sub));
                Ok(text(id))
            }
            "observe_status" | "observe_progress" | "observe_outcome" => {
                let key = if op == "observe_outcome" {
                    let f = exact(args, &["key"])?;
                    Some(string(&f["key"])?)
                } else {
                    exact(args, &[])?;
                    None
                };
                let id = allocate()?;
                let sub = match op {
                    "observe_status" => self.db.observe_queue_status()?,
                    "observe_progress" => self.db.observe_progress()?,
                    _ => self.db.observe_outcome(key.expect("outcome key"))?,
                };
                self.subscriptions.insert(id, Token::Sync(sub));
                Ok(text(id))
            }
            "unsubscribe" | "unsubscribe_sync" => {
                let f = exact(args, &["subscription"])?;
                let id = handle(&f["subscription"])?;
                let token = match self.subscriptions.get(&id).copied() {
                    Some(Token::Data(v)) if op == "unsubscribe" => Some(Token::Data(v)),
                    Some(Token::Sync(v)) if op == "unsubscribe_sync" => Some(Token::Sync(v)),
                    _ => None,
                };
                let Some(token) = token else {
                    return Ok(Json::Bool(false));
                };
                self.subscriptions.remove(&id);
                if let Some(events) = &mut self.events {
                    events.retain(|event| {
                        map(event).expect("owned event")["subscription"] != text(id)
                    });
                }
                Ok(Json::Bool(match token {
                    Token::Data(v) => self.db.unsubscribe(v),
                    Token::Sync(v) => self.db.unsubscribe_sync(v),
                }))
            }
            "refresh" => {
                exact(args, &[])?;
                run(async {
                    self.db.refresh_notifications().await;
                    Ok::<_, Error>(())
                })?;
                Ok(Json::Null)
            }
            "observation_stats" => {
                exact(args, &[])?;
                let s = self.db.observation_stats();
                Ok(object([
                    ("groups", text(s.groups)),
                    ("observers", text(s.observers)),
                    ("projected_queries", text(s.projected_queries)),
                    ("candidate_updates", text(s.candidate_updates)),
                    ("materialized_rows", text(s.materialized_rows)),
                ]))
            }
            "diagnostics" => {
                exact(args, &[])?;
                let d = self.db.diagnostics();
                Ok(object([
                    ("schema_version", text(d.schema_version)),
                    ("pending", text(d.pending)),
                    ("sending", text(d.sending)),
                    ("awaiting_outcome", text(d.awaiting_outcome)),
                    ("live", text(d.live)),
                    ("blocked", Json::Bool(d.blocked)),
                    ("ready", Json::Bool(d.ready)),
                    ("pages", text(d.pages)),
                    ("changes", text(d.changes)),
                    ("tombstones", text(d.tombstones)),
                ]))
            }
            "drain" => {
                let f = exact(args, &["now_ms"])?;
                let now = unsigned(&f["now_ms"])?;
                if self.events.is_none() {
                    let mut events = std::collections::VecDeque::new();
                    for event in self.db.drain_events(now) {
                        let (token, kind, value) = match event {
                            CommandEvent::Data(e) => {
                                let value = match e.value {
                                    Ok(ObservationValue::Rows(rows)) => {
                                        object([("kind", text("rows")), ("rows", rows_wire(&rows))])
                                    }
                                    Ok(ObservationValue::Count(count)) => {
                                        object([("kind", text("count")), ("count", text(count))])
                                    }
                                    Err(error) => object([
                                        ("kind", text("error")),
                                        ("error", object([("kind", text(Error::from(error).0))])),
                                    ]),
                                };
                                (Token::Data(e.subscription), "data", value)
                            }
                            CommandEvent::Status {
                                subscription,
                                value,
                            } => (Token::Sync(subscription), "status", status_wire(value)),
                            CommandEvent::Progress {
                                subscription,
                                value,
                            } => (Token::Sync(subscription), "progress", progress_wire(value)),
                            CommandEvent::Outcome {
                                subscription,
                                value,
                            } => (Token::Sync(subscription), "outcome", outcome_wire(&value)),
                        };
                        if let Some((id, _)) = self.subscriptions.iter().find(|(_, v)| **v == token)
                        {
                            events.push_back(object([
                                ("kind", text(kind)),
                                ("subscription", text(id)),
                                ("value", value),
                            ]));
                        }
                    }
                    self.events = Some(events);
                }
                let mut events = self.events.take().expect("captured delivery");
                let result = drain_page(&mut events, |id| self.terminate(id));
                if !events.is_empty() {
                    self.events = Some(events);
                }
                result
            }
            _ => Err(Error("InvalidRequest")),
        }
    }
}

fn page_frame(events: Vec<Json>, has_more: bool) -> Json {
    object([
        ("events", Json::Array(events)),
        ("has_more", Json::Bool(has_more)),
    ])
}

fn subscription_of(event: &Json) -> &Json {
    &map(event).expect("owned event")["subscription"]
}

/// SPEC-0036 item 1: one owned drain page. Only an event that would be the
/// first item of the page is tested alone; for that size calculation its own
/// subscription's later events are disregarded without being removed. An event
/// that cannot be a sole item terminates only its subscription: it and that
/// subscription's later events are removed, the core subscription ends and one
/// bounded terminal event takes its position. A non-first event that does not
/// fit ends the page normally.
fn drain_page(
    events: &mut VecDeque<Json>,
    mut terminate: impl FnMut(&Json) -> Result<()>,
) -> Result<Json> {
    let mut page = Vec::new();
    while let Some(head) = events.front() {
        if page.is_empty() {
            let id = subscription_of(head).clone();
            let others = events.iter().skip(1).any(|e| *subscription_of(e) != id);
            if budget::success(&page_frame(vec![head.clone()], others)).is_err() {
                events.retain(|e| *subscription_of(e) != id);
                terminate(&id)?;
                events.push_front(object([
                    ("kind", text("error")),
                    ("subscription", id),
                    ("value", object([("kind", text("LimitExceeded"))])),
                ]));
                continue;
            }
        }
        let mut candidate = page.clone();
        candidate.push(head.clone());
        if budget::success(&page_frame(candidate, events.len() > 1)).is_err() {
            break;
        }
        page.push(events.pop_front().expect("checked head"));
    }
    let value = page_frame(page, !events.is_empty());
    budget::success(&value)?;
    Ok(value)
}

impl Owner {
    /// Ends a terminated subscription's core registration and handle (SPEC-0036).
    fn terminate(&mut self, id: &Json) -> Result<()> {
        let id = handle(id)?;
        match self.subscriptions.remove(&id) {
            Some(Token::Data(v)) => {
                self.db.unsubscribe(v);
            }
            Some(Token::Sync(v)) => {
                self.db.unsubscribe_sync(v);
            }
            None => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, subscription: &str, value: Json) -> Json {
        object([
            ("kind", text(kind)),
            ("subscription", text(subscription)),
            ("value", value),
        ])
    }

    /// TC-0040 OD05: an oversized outcome event of sync subscription "7"
    /// terminates only that subscription; a recoverable data error event and a
    /// following status event are delivered unchanged in order.
    #[test]
    fn od05_oversized_sync_event_terminates_only_its_subscription() {
        let outcome = event(
            "outcome",
            "7",
            object([
                ("state", text("applied")),
                ("result", text("o".repeat(1_048_576))),
            ]),
        );
        let later_outcome = event("outcome", "7", object([("state", text("sent"))]));
        let recoverable = event(
            "data",
            "8",
            object([
                ("kind", text("error")),
                ("error", object([("kind", text("BackendFailure"))])),
            ]),
        );
        let status = event("status", "9", object([("pending", text("0"))]));
        let mut events = VecDeque::from(vec![
            outcome,
            recoverable.clone(),
            later_outcome,
            status.clone(),
        ]);
        let mut terminated = Vec::new();
        let page = drain_page(&mut events, |id| {
            terminated.push(id.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(terminated, vec![text("7")]);
        assert_eq!(
            page,
            page_frame(
                vec![
                    event("error", "7", object([("kind", text("LimitExceeded"))])),
                    recoverable,
                    status,
                ],
                false
            )
        );
        assert!(events.is_empty());
    }
}
