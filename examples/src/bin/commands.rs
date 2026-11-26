//! A Todo app's commands on the Rust core: create a task, then complete it.
//!
//! The core validates each command definition and admits one request scope at a time. The
//! application owns everything else: between `begin` and `finish` it calls its own handler,
//! which reads the admitted arguments, applies business rules and writes through the request.
//! Nothing is registered on a `CommandDefinition`; it only describes the command.
use hoyadb_doc_examples::*;

type TodoDb = CommandDatabase<MemoryStorage>;

const NOW_MS: i64 = 1_000;

// docs:start schema
fn todo_schema() -> SchemaDefinition {
    let tasks = TableDefinition {
        columns: BTreeMap::from([
            ("title".into(), column(ColumnType::String)),
            ("done".into(), column(ColumnType::Boolean)),
        ]),
        relations: BTreeMap::new(),
    };
    let activities = TableDefinition {
        columns: BTreeMap::from([
            ("task_id".into(), column(ColumnType::Reference)),
            ("kind".into(), column(ColumnType::String)),
        ]),
        relations: BTreeMap::new(),
    };
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([("tasks".into(), tasks), ("activities".into(), activities)]),
    }
}
// docs:end schema

// docs:start open
fn open_todo_app() -> TodoDb {
    let namespace = [7; 16]; // a fixed test namespace; let the platform choose one in an app
    run(CommandDatabase::open(
        MemoryStorage::default(),
        todo_schema(),
        vec![],
        namespace,
        SyncOptions::default(),
    ))
    .expect("the Todo schema is valid")
}
// docs:end open

// docs:start create-definition
fn create_task_definition() -> CommandDefinition {
    CommandDefinition {
        name: "create-task-v1".into(),
        arguments: BTreeMap::from([(
            "title".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        reference_paths: vec![],
    }
}
// docs:end create-definition

// docs:start complete-definition
fn complete_task_definition() -> CommandDefinition {
    CommandDefinition {
        name: "complete-task-v1".into(),
        arguments: BTreeMap::from([(
            "task_id".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        // task_id holds a row ID; it may name a task that another queued command created.
        reference_paths: vec![vec![PathPart::Key("task_id".into())]],
    }
}
// docs:end complete-definition

// docs:start errors
#[derive(Debug)]
enum TodoError {
    BlankTitle,
    UnknownTask,
    Database(CommandError),
}

impl From<CommandError> for TodoError {
    fn from(error: CommandError) -> Self {
        TodoError::Database(error)
    }
}

impl std::fmt::Display for TodoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoError::BlankTitle => write!(formatter, "a task needs a title"),
            TodoError::UnknownTask => write!(formatter, "there is no such task"),
            TodoError::Database(error) => {
                write!(formatter, "the database refused the change: {error:?}")
            }
        }
    }
}
// docs:end errors

// docs:start string-argument
/// Reads one string from the arguments the core admitted for this request.
fn string_argument(arguments: &Json, name: &str) -> Result<String, TodoError> {
    match arguments {
        Json::Object(values) => match values.get(name) {
            Some(Json::String(value)) => Ok(value.clone()),
            _ => Err(TodoError::Database(CommandError::InvalidCommand)),
        },
        _ => Err(TodoError::Database(CommandError::InvalidCommand)),
    }
}
// docs:end string-argument

// docs:start json-object
/// Builds a one-field JSON object such as {"task_id": "local:1"}.
fn json_object(name: &str, value: &str) -> Json {
    Json::Object(BTreeMap::from([(name.into(), Json::String(value.into()))]))
}
// docs:end json-object

// docs:start create-handler
/// create-task-v1: stores a task with a trimmed, nonblank title.
async fn create_task_handler(db: &mut TodoDb, request: CommandRequest) -> Result<Json, TodoError> {
    let title = string_argument(db.scope_args(request)?, "title")?;
    let title = title.trim();
    if title.is_empty() {
        return Err(TodoError::BlankTitle);
    }

    let task_fields = BTreeMap::from([
        ("title".into(), Value::String(title.into())),
        ("done".into(), Value::Boolean(false)),
    ]);
    let task = db.scope_insert(request, "tasks", task_fields).await?;

    Ok(json_object("task_id", &task.id))
}
// docs:end create-handler

// docs:start fetch-task
/// Reads one task through the request, so the handler sees its own staged writes.
async fn fetch_task(db: &TodoDb, request: CommandRequest, task_id: &str) -> Result<Row, TodoError> {
    let this_task = db
        .query("tasks")
        .where_("id", Operator::Eq, Value::String(task_id.into()));
    let mut found = db.scope_fetch(request, &this_task).await?;
    found.pop().ok_or(TodoError::UnknownTask)
}
// docs:end fetch-task

// docs:start complete-handler
/// complete-task-v1: marks the task done and records a "completed" activity, in one request.
async fn complete_task_handler(
    db: &mut TodoDb,
    request: CommandRequest,
) -> Result<Json, TodoError> {
    let task_id = string_argument(db.scope_args(request)?, "task_id")?;
    let task = fetch_task(db, request, &task_id).await?;

    let mut task_fields = task.fields;
    task_fields.insert("done".into(), Value::Boolean(true));
    db.scope_replace(request, "tasks", &task.id, task_fields)
        .await?;

    let activity_fields = BTreeMap::from([
        ("task_id".into(), Value::Reference(task.id.clone())),
        ("kind".into(), Value::String("completed".into())),
    ]);
    let activity = db
        .scope_insert(request, "activities", activity_fields)
        .await?;

    Ok(Json::Object(BTreeMap::from([
        ("task_id".into(), Json::String(task.id)),
        ("activity_id".into(), Json::String(activity.id)),
    ])))
}
// docs:end complete-handler

// docs:start finish-or-abort
/// Commits the request when the handler succeeded; otherwise discards everything it staged.
fn finish_or_abort(
    db: &mut TodoDb,
    request: CommandRequest,
    handled: Result<Json, TodoError>,
) -> Result<CommandResult, TodoError> {
    match handled {
        Ok(result) => Ok(run(db.finish(request, result))?),
        Err(error) => {
            db.abort(request)?;
            Err(error)
        }
    }
}
// docs:end finish-or-abort

// docs:start create-task
/// The application's entry point for "create a task": one new request per attempt.
fn create_task(db: &mut TodoDb, title: &str) -> Result<CommandResult, TodoError> {
    let request = db.request()?;
    let arguments = json_object("title", title);
    let identity = Some(Json::String("ada".into()));

    if !run(db.begin(
        request,
        create_task_definition(),
        arguments,
        identity,
        NOW_MS,
    ))? {
        db.cancel_request(request)?;
        return Err(CommandError::Busy.into());
    }
    let handled = run(create_task_handler(db, request));
    finish_or_abort(db, request, handled)
}
// docs:end create-task

// docs:start complete-task
/// The application's entry point for "complete a task": one new request per attempt.
fn complete_task(db: &mut TodoDb, task_id: &str) -> Result<CommandResult, TodoError> {
    let request = db.request()?;
    let arguments = json_object("task_id", task_id);
    let identity = Some(Json::String("ada".into()));

    if !run(db.begin(
        request,
        complete_task_definition(),
        arguments,
        identity,
        NOW_MS,
    ))? {
        db.cancel_request(request)?;
        return Err(CommandError::Busy.into());
    }
    let handled = run(complete_task_handler(db, request));
    finish_or_abort(db, request, handled)
}
// docs:end complete-task

/// Reads a string field from a handler's JSON result.
fn result_id(result: &Json, field: &str) -> String {
    string_argument(result, field).expect("the handler returned this ID")
}

// docs:start confirm
/// Records the server's answer for one command, then maps the row the command created to the ID
/// named in that answer.
fn confirm(db: &mut TodoDb, key: &str, local_id: &str, server_answer: Json, id_field: &str) {
    let server_id = result_id(&server_answer, id_field);
    db.receive(key, Some(RemoteOutcome::Applied(server_answer)))
        .expect("a sent command");
    let plan = ReconcilePlan {
        writes: vec![],
        mappings: BTreeMap::from([(local_id.to_string(), server_id)]),
    };
    run(db.reconcile(key, plan)).expect("the mapping covers the created row");
}
// docs:end confirm

fn rows(db: &TodoDb, table: &str) -> Vec<Row> {
    run(db.query(table).fetch(db)).unwrap()
}

fn main() {
    let mut db = open_todo_app();

    // docs:start run-create
    let created = create_task(&mut db, "  Buy milk  ").expect("a nonblank title");
    let task_id = result_id(&created.result, "task_id");
    // created.key identifies the queued command; created.result is the handler's JSON.
    // docs:end run-create
    assert!(task_id.starts_with("local:"));
    assert_ne!(created.key, task_id);
    assert_eq!(db.queue_status().commands[0].key, created.key);
    assert_eq!(
        rows(&db, "tasks")[0].fields["title"],
        Value::String("Buy milk".into())
    );

    // docs:start run-complete
    let completed = complete_task(&mut db, &task_id).expect("the task exists locally");
    let activity_id = result_id(&completed.result, "activity_id");
    // Both writes are visible at once; nothing has been sent to the server yet.
    // docs:end run-complete
    assert_eq!(rows(&db, "tasks")[0].fields["done"], Value::Boolean(true));
    assert_eq!(rows(&db, "activities")[0].id, activity_id);
    assert_eq!(db.queue_status().live, 2);
    assert!(
        db.queue_status()
            .commands
            .iter()
            .all(|entry| entry.state == SyncState::Queued)
    );

    // docs:start first-push
    let batch = db.prepare_push(NOW_MS, 0).unwrap();
    // Only create-task-v1 is sent: complete-task-v1 waits until the task has its server ID.
    // docs:end first-push
    assert_eq!(batch.envelopes.len(), 1);
    assert!(batch.envelopes[0].contains("create-task-v1"));
    let retry = db.prepare_push(batch.next_due_ms.unwrap(), 0).unwrap();
    assert_eq!(
        batch.envelopes, retry.envelopes,
        "a retry resends the same frozen envelope"
    );

    // docs:start confirm-create
    // Stand-in for the server's answer to create-task-v1, decoded by your transport.
    let server_answer = json_object("task_id", "server-task-1");
    confirm(&mut db, &created.key, &task_id, server_answer, "task_id");
    // docs:end confirm-create
    assert_eq!(rows(&db, "tasks")[0].id, "server-task-1");
    assert_eq!(
        rows(&db, "activities")[0].fields["task_id"],
        Value::Reference("server-task-1".into())
    );

    // docs:start second-push
    let batch = db.prepare_push(NOW_MS, 0).unwrap();
    // complete-task-v1 is sent now, and its task_id argument is "server-task-1".
    // docs:end second-push
    assert_eq!(batch.envelopes.len(), 1);
    assert!(batch.envelopes[0].contains("complete-task-v1"));
    assert!(batch.envelopes[0].contains("\"task_id\":\"server-task-1\""));

    // docs:start confirm-complete
    // Stand-in for the server's answer to complete-task-v1.
    let server_answer = json_object("activity_id", "server-activity-1");
    confirm(
        &mut db,
        &completed.key,
        &activity_id,
        server_answer,
        "activity_id",
    );
    // docs:end confirm-complete
    assert_eq!(db.queue_status().live, 0);
    assert_eq!(rows(&db, "activities")[0].id, "server-activity-1");
    assert_eq!(rows(&db, "tasks")[0].fields["done"], Value::Boolean(true));
    println!("PASS commands: staged commit, frozen retry, receive and reconcile");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_keys(db: &TodoDb) -> Vec<String> {
        db.queue_status()
            .commands
            .into_iter()
            .map(|entry| entry.key)
            .collect()
    }

    #[test]
    fn create_trims_the_title_and_returns_its_chosen_result() {
        let mut db = open_todo_app();
        let created = create_task(&mut db, "  Buy milk  ").unwrap();
        let tasks = rows(&db, "tasks");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].fields["title"], Value::String("Buy milk".into()));
        assert_eq!(result_id(&created.result, "task_id"), tasks[0].id);
        assert!(!created.key.is_empty() && created.key != tasks[0].id);
        assert_eq!(queue_keys(&db), vec![created.key]);
    }

    #[test]
    fn blank_title_is_rejected_without_a_row_or_queue_record() {
        let mut db = open_todo_app();
        let rejected = create_task(&mut db, "   ");
        assert!(matches!(rejected, Err(TodoError::BlankTitle)));
        assert_eq!(rejected.unwrap_err().to_string(), "a task needs a title");
        assert!(rows(&db, "tasks").is_empty());
        assert_eq!(db.queue_status().live, 0);
        // The aborted request is gone; a new attempt takes a new request.
        assert!(create_task(&mut db, "Buy milk").is_ok());
    }

    #[test]
    fn completion_updates_and_inserts_in_one_commit() {
        let mut db = open_todo_app();
        let created = create_task(&mut db, "Buy milk").unwrap();
        let task_id = result_id(&created.result, "task_id");
        let completed = complete_task(&mut db, &task_id).unwrap();
        assert_eq!(result_id(&completed.result, "task_id"), task_id);
        assert_eq!(rows(&db, "tasks")[0].fields["done"], Value::Boolean(true));
        let activities = rows(&db, "activities");
        assert_eq!(activities.len(), 1);
        assert_eq!(
            activities[0].id,
            result_id(&completed.result, "activity_id")
        );
        assert_eq!(activities[0].fields["task_id"], Value::Reference(task_id));
        assert_eq!(
            activities[0].fields["kind"],
            Value::String("completed".into())
        );
        assert_eq!(
            queue_keys(&db),
            vec![created.key, completed.key],
            "one more queue record"
        );
    }

    #[test]
    fn completion_declares_task_id_as_a_reference() {
        assert_eq!(
            complete_task_definition().reference_paths,
            vec![vec![PathPart::Key("task_id".into())]]
        );
    }

    #[test]
    fn late_failure_aborts_both_writes_and_a_new_request_succeeds() {
        let mut db = open_todo_app();
        let first = create_task(&mut db, "Buy milk").unwrap();
        let first_id = result_id(&first.result, "task_id");
        let first_done = complete_task(&mut db, &first_id).unwrap();
        let existing_activity = result_id(&first_done.result, "activity_id");
        let second = create_task(&mut db, "Walk the dog").unwrap();
        let second_id = result_id(&second.result, "task_id");
        let (tasks, activities, keys) =
            (rows(&db, "tasks"), rows(&db, "activities"), queue_keys(&db));
        let status = db.queue_status();

        // The next generated ID collides with the existing activity, so the activity insert
        // fails after the task update was already staged.
        db.set_id_generator(move || existing_activity.clone())
            .unwrap();
        let failed = complete_task(&mut db, &second_id);
        assert!(
            matches!(
                failed,
                Err(TodoError::Database(CommandError::Storage(
                    StorageError::RowAlreadyExists { .. }
                )))
            ),
            "the activity insert itself fails: {failed:?}"
        );
        assert_eq!(rows(&db, "tasks"), tasks, "the staged update was discarded");
        assert_eq!(rows(&db, "activities"), activities);
        assert_eq!(queue_keys(&db), keys);
        // QueueStatus has no Debug, so compare it with ==.
        assert!(
            db.queue_status() == status,
            "the whole queue status is unchanged"
        );

        db.set_id_generator(|| "local:activity-retry".into())
            .unwrap();
        let retried = complete_task(&mut db, &second_id).unwrap();
        assert_eq!(
            result_id(&retried.result, "activity_id"),
            "local:activity-retry"
        );
        assert_eq!(queue_keys(&db).len(), keys.len() + 1);
        let second_task = rows(&db, "tasks")
            .into_iter()
            .find(|task| task.id == second_id)
            .unwrap();
        assert_eq!(second_task.fields["done"], Value::Boolean(true));
    }

    #[test]
    fn completion_waits_for_creation_and_sends_the_mapped_id() {
        let mut db = open_todo_app();
        let created = create_task(&mut db, "Buy milk").unwrap();
        let task_id = result_id(&created.result, "task_id");
        let completed = complete_task(&mut db, &task_id).unwrap();
        let first = db.prepare_push(NOW_MS, 0).unwrap();
        assert_eq!(first.envelopes.len(), 1);
        assert!(first.envelopes[0].contains("create-task-v1"));
        let waiting = db
            .queue_status()
            .commands
            .into_iter()
            .find(|entry| entry.key == completed.key)
            .unwrap();
        assert_eq!(waiting.state, SyncState::Queued);

        // Stand-in for the server's answer to create-task-v1, decoded by your transport.
        let server_answer = json_object("task_id", "server-task-1");
        confirm(&mut db, &created.key, &task_id, server_answer, "task_id");
        let second = db.prepare_push(NOW_MS, 0).unwrap();
        assert_eq!(second.envelopes.len(), 1);
        assert!(second.envelopes[0].contains("complete-task-v1"));
        assert!(second.envelopes[0].contains("\"task_id\":\"server-task-1\""));
        assert!(!second.envelopes[0].contains(&task_id));
    }
}
