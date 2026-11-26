//! API-0009 section 6: experimental boundary ownership, never a second sync engine.
use super::*;

pub(super) struct PendingPull {
    request: PullRequest,
    body: String,
}

fn captured(args: &BTreeMap<String, Json>) -> Result<EntityContext> {
    Ok(EntityContext {
        identity: unbox(field(args, "identity")?)?,
        now_ms: integer(field(args, "now_ms")?)?,
    })
}

fn with_context<T>(
    owner: &mut Owner,
    context: EntityContext,
    call: impl FnOnce(&mut Owner) -> Result<T>,
) -> Result<T> {
    *owner.context.lock().map_err(|_| Error("Closed"))? = Some(context);
    let result = call(owner);
    *owner.context.lock().map_err(|_| Error("Closed"))? = None;
    result
}

pub(super) fn operation(owner: &mut Owner, op: &str, args: &Json) -> Result<Json> {
    match op {
        "entity_configure" => {
            let f = exact(args, &["scope", "tables"])?;
            let names = array(field(f, "tables")?)?
                .iter()
                .map(|v| string(v).map(str::to_owned))
                .collect::<Result<Vec<_>>>()?;
            let tables = names.iter().cloned().collect::<BTreeSet<_>>();
            if names.len() != tables.len() {
                return Err(Error("InvalidRequest"));
            }
            let context = owner.context.clone();
            owner.db.configure_entities(
                EntityConfig {
                    scope: string(field(f, "scope")?)?.into(),
                    tables,
                },
                move || {
                    context
                        .lock()
                        .map_err(|_| CommandError::Closed)?
                        .clone()
                        .ok_or(CommandError::InvalidCommand)
                },
            )?;
            Ok(Json::Null)
        }
        "entity_batch" => {
            let f = exact(args, &["writes", "identity", "now_ms"])?;
            let context = captured(f)?;
            let writes = writes(field(f, "writes")?)?;
            budget::entity_batch(&writes, |table, id| {
                let q = owner
                    .db
                    .query(table)
                    .where_("id", Operator::Eq, Value::String(id.into()));
                run_once(q.fetch(&owner.db))
            })?;
            with_context(owner, context, |owner| {
                Ok(changes_wire(&run_once(owner.db.apply_batch(writes))?))
            })
        }
        "entity_patch" => {
            let f = exact(args, &["table", "id", "fields", "identity", "now_ms"])?;
            let context = captured(f)?;
            let table = string(field(f, "table")?)?;
            let id = string(field(f, "id")?)?;
            let fields = row_fields(field(f, "fields")?)?;
            let q = owner
                .db
                .query(table)
                .where_("id", Operator::Eq, Value::String(id.into()));
            let mut expected = run_once(q.fetch(&owner.db))?
                .into_iter()
                .next()
                .ok_or(Error("RowNotFound"))?
                .fields;
            expected.extend(fields.clone());
            budget::row(Some(id), &expected)?;
            with_context(owner, context, |owner| {
                Ok(row_wire(&run_once(owner.db.patch(table, id, fields))?))
            })
        }
        "receive_entity" => {
            let f = exact(args, &["table", "record_id", "write_id", "outcome"])?;
            let received = field(f, "outcome")?;
            let received = if received == &Json::Null {
                None
            } else {
                Some(outcome(received)?)
            };
            owner.db.receive_entity(
                string(field(f, "table")?)?,
                string(field(f, "record_id")?)?,
                string(field(f, "write_id")?)?,
                received,
            )?;
            Ok(Json::Null)
        }
        "begin_pull" => {
            exact(args, &[])?;
            let token = allocate()?;
            let batch = owner.db.begin_pull()?;
            let value = object([
                ("request", text(token)),
                ("since", batch.since.map(text).unwrap_or(Json::Null)),
                ("scope", text(batch.scope)),
                ("generation", text(batch.generation)),
            ]);
            if let Err(error) = budget::success(&value) {
                owner.db.abort_pull(batch.request)?;
                return Err(error);
            }
            owner.pulls.insert(
                token,
                PendingPull {
                    request: batch.request,
                    body: String::new(),
                },
            );
            Ok(value)
        }
        "pull_chunk" => {
            let f = exact(args, &["request", "offset", "chunk"])?;
            let token = handle(field(f, "request")?)?;
            let offset = unsigned(field(f, "offset")?)?;
            let chunk = string(field(f, "chunk")?)?;
            let pull = owner.pulls.get_mut(&token).ok_or(Error("InvalidHandle"))?;
            if offset != pull.body.len() as u64 {
                return Err(Error("InvalidRequest"));
            }
            let size = pull
                .body
                .len()
                .checked_add(chunk.len())
                .ok_or(Error("LimitExceeded"))?;
            if chunk.len() > 262144 || size > 1048576 {
                return Err(Error("LimitExceeded"));
            }
            pull.body.push_str(chunk);
            Ok(text(size))
        }
        "apply_pull" => {
            let f = exact(args, &["request"])?;
            let token = handle(field(f, "request")?)?;
            let pull = owner.pulls.get(&token).ok_or(Error("InvalidHandle"))?;
            let page = PullPage::from_json(
                &Json::parse(&pull.body).map_err(|_| Error("InvalidPayload"))?,
            )?;
            let result = run_once(owner.db.apply_pull(pull.request, page));
            if result.is_ok() || matches!(&result, Err(Error("StalePull"))) {
                owner.pulls.remove(&token);
            }
            result?;
            Ok(Json::Null)
        }
        "abort_pull" => {
            let f = exact(args, &["request"])?;
            let token = handle(field(f, "request")?)?;
            let request = owner
                .pulls
                .get(&token)
                .ok_or(Error("InvalidHandle"))?
                .request;
            owner.db.abort_pull(request)?;
            owner.pulls.remove(&token);
            Ok(Json::Null)
        }
        "sync_progress" => {
            exact(args, &[])?;
            Ok(progress_wire(owner.db.sync_progress()))
        }
        "observe_progress" => {
            exact(args, &[])?;
            let token = allocate()?;
            let subscription = owner.db.observe_progress()?;
            owner.subscriptions.insert(token, Token::Sync(subscription));
            Ok(text(token))
        }
        "reset" => {
            let f = exact(args, &["mode"])?;
            let mode = match string(field(f, "mode")?)? {
                "preserve" => ResetMode::PreserveRecovery,
                "discard" => ResetMode::DiscardRecovery,
                _ => return Err(Error("InvalidRequest")),
            };
            run_once(owner.db.reset(mode))?;
            owner.requests.clear();
            owner.pulls.clear();
            owner.subscriptions.clear();
            owner.events = None;
            owner.push = None;
            Ok(Json::Null)
        }
        _ => Err(Error("InvalidRequest")),
    }
}
