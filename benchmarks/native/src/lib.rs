//! TC-0037: std-only driver of the real general facade and caller fixture A.
use hoyadb_bindings::Facade;
use hoyadb_core::command::Json;
use std::{collections::BTreeMap, path::Path, time::Instant};
pub type Result<T> = std::result::Result<T, String>;
pub fn object<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn text(value: impl ToString) -> Json {
    Json::String(value.to_string())
}
fn map(value: &Json) -> Result<&BTreeMap<String, Json>> {
    if let Json::Object(value) = value {
        Ok(value)
    } else {
        Err("expected object".into())
    }
}
fn list(value: &Json) -> Result<&Vec<Json>> {
    if let Json::Array(value) = value {
        Ok(value)
    } else {
        Err("expected array".into())
    }
}
fn tag(kind: &str, value: Json) -> Json {
    object([("kind", text(kind)), ("value", value)])
}
fn check(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
pub fn schema() -> Result<Json> {
    let value = Json::parse(include_str!("../../workloads.json")).map_err(|_| "workload JSON")?;
    Ok(map(&value)?.get("schema").ok_or("workload schema")?.clone())
}
fn id(kind: &str, index: usize) -> String {
    format!("{kind}:{index:06}")
}
fn article(index: usize) -> Json {
    let mut fields = BTreeMap::from([
        (
            "title".into(),
            tag("string", text(format!("東京🌿-{index}"))),
        ),
        ("score".into(), tag("integer", text(index))),
        ("ratio".into(), tag("float", text(format!("{index}.25")))),
        (
            "active".into(),
            tag("boolean", Json::Bool(index.is_multiple_of(2))),
        ),
        (
            "at".into(),
            tag("timestamp", text(1_700_000_000_000u64 + index as u64)),
        ),
        (
            "author_id".into(),
            tag("reference", text(id("author", index / 10))),
        ),
    ]);
    if index % 2 == 1 {
        fields.insert("note".into(), tag("string", text(format!("note-{index}"))));
    }
    Json::Object(fields)
}
fn row(index: usize) -> Json {
    object([
        ("id", text(id("article", index))),
        ("fields", article(index)),
    ])
}
fn query(table: &str, predicates: Vec<Json>, order: Option<(&str, &str)>, limit: usize) -> Json {
    object([
        ("table", text(table)),
        ("predicates", Json::Array(predicates)),
        (
            "order",
            order
                .map(|(field, direction)| {
                    object([("field", text(field)), ("direction", text(direction))])
                })
                .unwrap_or(Json::Null),
        ),
        ("limit", text(limit)),
        ("source", Json::Null),
    ])
}
fn predicate(field: &str, operator: &str, value: Json) -> Json {
    object([
        ("field", text(field)),
        ("operator", text(operator)),
        ("values", Json::Array(vec![value])),
    ])
}
fn insert(table: &str, id: String, fields: Json) -> Json {
    object([
        ("op", text("insert")),
        ("table", text(table)),
        ("fields", fields),
        ("proposed_id", text(id)),
    ])
}
fn replace(index: usize, fields: Json) -> Json {
    object([
        ("op", text("replace")),
        ("table", text("articles")),
        ("id", text(id("article", index))),
        ("fields", fields),
    ])
}
fn remove(index: usize) -> Json {
    object([
        ("op", text("remove")),
        ("table", text("articles")),
        ("id", text(id("article", index))),
    ])
}
fn decode(output: &str) -> Result<Json> {
    let response = Json::parse(output).map_err(|_| "invalid response JSON")?;
    let response = map(&response)?;
    if response.get("ok") != Some(&Json::Bool(true)) {
        let kind = response
            .get("error")
            .and_then(|v| map(v).ok())
            .and_then(|v| v.get("kind"));
        return Err(match kind {
            Some(Json::String(kind)) => format!("facade {kind}"),
            _ => "facade error".into(),
        });
    }
    response
        .get("value")
        .cloned()
        .ok_or("missing response value".into())
}
fn verify_batch_result(operation: &str, n: usize, iteration: usize, value: &Json) -> Result<()> {
    let mut changes = Vec::new();
    for index in 0..10 {
        let (identifier, before, after) = match operation {
            "batch-insert" => {
                let identifier = id("article", n + index);
                (
                    identifier.clone(),
                    Json::Null,
                    object([
                        ("id", text(identifier)),
                        ("fields", article(n - 10 + index)),
                    ]),
                )
            }
            "batch-update" => {
                let mut fields = article(index);
                let Json::Object(ref mut fields_map) = fields else {
                    return Err("article object".into());
                };
                fields_map.insert(
                    "title".into(),
                    tag("string", text(format!("changed-{iteration}-{index}"))),
                );
                (
                    id("article", index),
                    row(index),
                    object([("id", text(id("article", index))), ("fields", fields)]),
                )
            }
            "batch-remove" => (id("article", index), row(index), Json::Null),
            _ => return Err("unknown batch proof".into()),
        };
        changes.push(object([
            ("table", text("articles")),
            ("id", text(identifier)),
            ("before", before),
            ("after", after),
        ]));
    }
    check(
        *value == Json::Array(changes),
        "batch exact ID/before/after mismatch",
    )
}
pub enum ResourceEvent<'a> {
    Checkpoint(&'static str, Option<usize>),
    ProofBegin(&'static str),
    ProofRow(&'a str, &'a Json),
    ProofEnd(&'static str),
}
pub fn rss_cycles(
    directory: Option<&Path>,
    mut emit: impl FnMut(ResourceEvent<'_>) -> Result<()>,
) -> Result<Json> {
    emit(ResourceEvent::Checkpoint("baseline", None))?;
    let mut protection = Json::Null;
    for cycle in 0..30 {
        {
            let owned = directory.map(|root| root.join(format!("cycle-{cycle:02}")));
            if let Some(path) = &owned {
                std::fs::create_dir(path).map_err(|_| "resource owned directory")?;
            }
            let mut harness = Harness::new(100, owned.as_deref())?;
            harness.set_observers(1, "equivalent", "membership")?;
            if protection == Json::Null {
                protection = harness.protection.clone();
            } else {
                check(
                    protection == harness.protection,
                    "resource protection changed",
                )?;
            }
            emit(ResourceEvent::Checkpoint("after_open", Some(cycle)))?;
            if cycle == 0 || cycle == 29 {
                let phase = if cycle == 0 {
                    "initial"
                } else {
                    "final"
                };
                emit(ResourceEvent::ProofBegin(phase))?;
                harness.verify_to(|table, row| emit(ResourceEvent::ProofRow(table, row)))?;
                emit(ResourceEvent::ProofEnd(phase))?;
            } else {
                harness.verify()?;
            }
        }
        emit(ResourceEvent::Checkpoint("after_close", Some(cycle)))?;
    }
    Ok(protection)
}
pub struct Harness {
    facade: Facade,
    owner: Json,
    n: usize,
    input_bytes: usize,
    output_bytes: usize,
    origin: Instant,
    observers: Vec<(Json, Json)>,
    mode: String,
    configured: bool,
    queue: usize,
    baseline_status: Json,
    epoch: usize,
    clock: i64,
    pub open_ns: u64,
    pub protection: Json,
}
impl Harness {
    pub fn new(n: usize, directory: Option<&Path>) -> Result<Self> {
        check([100, 1000, 10000].contains(&n), "unsupported dataset")?;
        let mut this = Self::cold(n, directory, false)?;
        let mut writes = Vec::new();
        for index in 0..n / 10 {
            writes.push(insert(
                "authors",
                id("author", index),
                object([("name", tag("string", text(format!("author-{index:06}"))))]),
            ));
        }
        this.seed(writes)?;
        this.seed(
            (0..n)
                .map(|index| insert("articles", id("article", index), article(index)))
                .collect(),
        )?;
        Ok(this)
    }
    pub fn cold(n: usize, directory: Option<&Path>, upgrade: bool) -> Result<Self> {
        check([0, 100, 1000, 10000].contains(&n), "unsupported dataset")?;
        check(
            !upgrade || (n > 0 && directory.is_some()),
            "invalid upgrade class",
        )?;
        let mut definition = schema()?;
        let migrations = if upgrade {
            let Json::Object(top) = &mut definition else {
                return Err("schema object".into());
            };
            top.insert("version".into(), text(2));
            let Json::Object(tables) = top.get_mut("tables").ok_or("schema tables")? else {
                return Err("schema tables".into());
            };
            let Json::Object(articles) = tables.get_mut("articles").ok_or("schema articles")?
            else {
                return Err("schema articles".into());
            };
            let Json::Object(columns) = articles.get_mut("columns").ok_or("schema columns")? else {
                return Err("schema columns".into());
            };
            let column = object([("kind", text("String")), ("optional", Json::Bool(true))]);
            columns.insert("benchmark_extra".into(), column.clone());
            Json::Array(vec![object([
                ("from_version", text(1)),
                ("to", definition.clone()),
                (
                    "steps",
                    Json::Array(vec![object([
                        ("op", text("AddColumn")),
                        ("table", text("articles")),
                        ("name", text("benchmark_extra")),
                        ("column", column),
                        ("default", object([("kind", text("none"))])),
                    ])]),
                ),
            ])])
        } else {
            Json::Array(vec![])
        };
        let engine = match directory {
            None => object([("kind", text("memory")), ("store", Json::Null)]),
            Some(directory) => object([
                ("kind", text("native")),
                (
                    "directory",
                    text(directory.to_str().ok_or("non UTF8 path")?),
                ),
                ("name", text("benchmark.sqlite")),
                ("backup_excluded", Json::Bool(false)),
            ]),
        };
        let args = object([
            ("engine", engine),
            ("schema", definition),
            ("migrations", migrations),
            ("namespace", text("55555555555555555555555555555555")),
            (
                "options",
                object([
                    ("capacity", text(1000)),
                    ("history", text(100)),
                    ("initial_delay_ms", text(1000)),
                    ("multiplier", text(2)),
                    ("max_delay_ms", text(60000)),
                ]),
            ),
        ]);
        let mut facade = Facade::new();
        let started = Instant::now();
        let input = object([
            ("abi", text(1)),
            ("op", text("open")),
            ("owner", Json::Null),
            ("args", args),
        ])
        .canonical()
        .map_err(|_| "open encoding")?;
        let owner = decode(&facade.open(
            &input,
            if directory.is_some() {
                vec![0x53; 32]
            } else {
                vec![]
            },
        ))?;
        let open_ns =
            u64::try_from(started.elapsed().as_nanos()).map_err(|_| "duration overflow")?;
        let mut this = Self {
            facade,
            owner,
            n,
            input_bytes: 0,
            output_bytes: 0,
            open_ns,
            origin: Instant::now(),
            observers: vec![],
            mode: String::new(),
            configured: false,
            queue: 0,
            baseline_status: Json::Null,
            epoch: 0,
            clock: 2000,
            protection: Json::Null,
        };
        this.protection = this.call("protection", object([]))?;
        check(
            map(&this.protection)?["encrypted"] == Json::Bool(directory.is_some()),
            "unexpected protection",
        )?;
        Ok(this)
    }
    pub fn call(&mut self, op: &str, args: Json) -> Result<Json> {
        let input = object([
            ("abi", text(1)),
            ("op", text(op)),
            ("owner", self.owner.clone()),
            ("args", args),
        ])
        .canonical()
        .map_err(|_| "request encoding")?;
        self.input_bytes += input.len();
        let output = self.facade.call(&input);
        self.output_bytes += output.len();
        decode(&output)
    }
    fn seed(&mut self, writes: Vec<Json>) -> Result<()> {
        for chunk in writes.chunks(100) {
            let result = self.call("batch", object([("writes", Json::Array(chunk.to_vec()))]))?;
            check(list(&result)?.len() == chunk.len(), "seed count")?;
        }
        Ok(())
    }
    pub fn verify(&mut self) -> Result<()> {
        self.verify_to(|_, _| Ok(()))
    }
    pub fn verify_to(&mut self, mut consume: impl FnMut(&str, &Json) -> Result<()>) -> Result<()> {
        let mut index = 0;
        loop {
            let values = self.call(
                "fetch",
                object([(
                    "query",
                    query(
                        "articles",
                        vec![predicate(
                            "score",
                            "Gt",
                            tag("integer", text(index as i64 - 1)),
                        )],
                        Some(("score", "Asc")),
                        100,
                    ),
                )]),
            )?;
            let values = list(&values)?;
            if values.is_empty() {
                break;
            }
            check(values.len() <= 100, "verification page overflow")?;
            for value in values {
                check(
                    index < self.n && *value == row(index),
                    "article fixture mismatch",
                )?;
                consume("articles", value)?;
                index += 1;
            }
        }
        check(index == self.n, "article cardinality")?;
        let mut index = 0;
        let mut last = String::new();
        loop {
            let values = self.call(
                "fetch",
                object([(
                    "query",
                    query(
                        "authors",
                        vec![predicate("id", "Gt", tag("string", text(&last)))],
                        Some(("id", "Asc")),
                        100,
                    ),
                )]),
            )?;
            let values = list(&values)?;
            if values.is_empty() {
                break;
            }
            for value in values {
                last = id("author", index);
                check(
                    *value
                        == object([
                            ("id", text(&last)),
                            (
                                "fields",
                                object([(
                                    "name",
                                    tag("string", text(format!("author-{index:06}"))),
                                )]),
                            ),
                        ]),
                    "author fixture mismatch",
                )?;
                consume("authors", value)?;
                index += 1;
            }
        }
        check(index == self.n / 10, "author cardinality")?;
        for table in ["tags", "article_tags"] {
            let values = self.call(
                "fetch",
                object([("query", query(table, vec![], None, 100))]),
            )?;
            check(list(&values)?.is_empty(), "unexpected graph row")?;
            consume(table, &Json::Null)?;
        }
        Ok(())
    }
    pub fn sample(&mut self, operation: &str, iteration: usize) -> Result<Json> {
        match operation {
            "boundary-known" => return self.boundary_sample(),
            "reactivity" => return self.reactive_sample(iteration),
            "optimistic-command" => return self.command_sample(iteration),
            "entity-patch" => return self.entity_sample(iteration),
            "non-noop-pull" => return self.pull_sample(iteration),
            "reset" => return self.reset_sample(),
            _ => {}
        }
        let args = match operation {
            "point" => object([(
                "query",
                query(
                    "articles",
                    vec![predicate(
                        "id",
                        "Eq",
                        tag("string", text(id("article", self.n / 2))),
                    )],
                    None,
                    1,
                ),
            )]),
            "selective" => object([(
                "query",
                query(
                    "articles",
                    vec![predicate("score", "Eq", tag("integer", text(self.n / 2)))],
                    None,
                    1,
                ),
            )]),
            "sorted-related" => object([(
                "query",
                query("articles", vec![], Some(("author.name", "Asc")), 10),
            )]),
            "batch-insert" => object([(
                "writes",
                Json::Array(
                    (0..10)
                        .map(|index| {
                            insert(
                                "articles",
                                id("article", self.n + index),
                                article(self.n - 10 + index),
                            )
                        })
                        .collect(),
                ),
            )]),
            "batch-update" => object([(
                "writes",
                Json::Array(
                    (0..10)
                        .map(|index| {
                            let mut fields = article(index);
                            if let Json::Object(fields) = &mut fields {
                                fields.insert(
                                    "title".into(),
                                    tag("string", text(format!("changed-{iteration}-{index}"))),
                                );
                            }
                            replace(index, fields)
                        })
                        .collect(),
                ),
            )]),
            "batch-remove" => object([("writes", Json::Array((0..10).map(remove).collect()))]),
            _ => return Err("operation pending".into()),
        };
        self.input_bytes = 0;
        self.output_bytes = 0;
        let start = Instant::now();
        let value = self.call(
            if operation.starts_with("batch-") {
                "batch"
            } else {
                "fetch"
            },
            args,
        )?;
        let elapsed = u64::try_from(start.elapsed().as_nanos()).map_err(|_| "duration overflow")?;
        let input_bytes = self.input_bytes;
        let output_bytes = self.output_bytes;
        if operation.starts_with("batch-") {
            verify_batch_result(operation, self.n, iteration, &value)?;
        }
        match operation {
            "point" | "selective" => {
                check(value == Json::Array(vec![row(self.n / 2)]), "point result")?
            }
            "sorted-related" => check(
                value == Json::Array((0..10).map(row).collect()),
                "relation window",
            )?,
            "batch-insert" => {
                let writes = (self.n..self.n + 10).map(remove).collect();
                self.call("batch", object([("writes", Json::Array(writes))]))?;
            }
            "batch-update" => {
                self.call(
                    "batch",
                    object([(
                        "writes",
                        Json::Array(
                            (0..10)
                                .map(|index| replace(index, article(index)))
                                .collect(),
                        ),
                    )]),
                )?;
            }
            "batch-remove" => {
                self.seed(
                    (0..10)
                        .map(|index| insert("articles", id("article", index), article(index)))
                        .collect(),
                )?;
            }
            _ => return Err("unverified operation".into()),
        }
        Ok(object([
            (
                "value",
                Json::Integer(i64::try_from(elapsed).map_err(|_| "duration overflow")?),
            ),
            ("status", text("ok")),
            ("correct", Json::Bool(true)),
            ("input_bytes", Json::Integer(input_bytes as i64)),
            ("output_bytes", Json::Integer(output_bytes as i64)),
        ]))
    }
    pub fn set_observers(&mut self, fanout: usize, sharing: &str, mode: &str) -> Result<()> {
        check(
            [0, 1, 10, 100].contains(&fanout)
                && ["equivalent", "distinct"].contains(&sharing)
                && ["membership", "columns", "count"].contains(&mode),
            "invalid observer config",
        )?;
        for (subscription, _) in std::mem::take(&mut self.observers) {
            self.call("unsubscribe", object([("subscription", subscription)]))?;
        }
        self.mode = mode.into();
        for index in 0..fanout {
            let mut q = query(
                "articles",
                vec![predicate(
                    "score",
                    "Ge",
                    tag(
                        "integer",
                        text(if sharing == "distinct" {
                            index
                        } else {
                            0
                        }),
                    ),
                )],
                Some(("score", "Desc")),
                10,
            );
            if mode == "count"
                && let Json::Object(q) = &mut q
            {
                q.insert("limit".into(), Json::Null);
            }
            let subscription = self.call(
                "observe",
                object([
                    ("query", q.clone()),
                    ("mode", text(mode)),
                    (
                        "columns",
                        Json::Array(if mode == "columns" {
                            vec![text("title")]
                        } else {
                            vec![]
                        }),
                    ),
                ]),
            )?;
            self.observers.push((subscription, q));
        }
        if fanout > 0 {
            let now = Instant::now();
            let initial = self.wait_events(now, now)?;
            check(initial.len() == fanout, "initial callback count")?;
        }
        Ok(())
    }
    fn wait_events(&mut self, start: Instant, durable: Instant) -> Result<Vec<(Json, u64, u64)>> {
        let mut observed = Vec::new();
        let expected = self.observers.len();
        let wait = Instant::now();
        while observed.len() < expected {
            let result = self.call(
                "drain",
                object([("now_ms", text(self.origin.elapsed().as_millis()))]),
            )?;
            for event in list(&map(&result)?["events"])? {
                let mut callback = |event: Json| {
                    let at = Instant::now();
                    observed.push((
                        event,
                        at.duration_since(start).as_nanos() as u64,
                        at.duration_since(durable).as_nanos() as u64,
                    ))
                };
                callback(event.clone());
            }
            check(observed.len() <= expected, "duplicate callback")?;
            if observed.len() < expected {
                check(wait.elapsed().as_millis() < 60000, "callback watchdog")?;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        Ok(observed)
    }
    fn reactive_sample(&mut self, iteration: usize) -> Result<Json> {
        check(!self.observers.is_empty(), "missing observers")?;
        let (op, args) = if self.mode == "columns" {
            let mut fields = article(self.n - 1);
            if let Json::Object(fields) = &mut fields {
                fields.insert(
                    "title".into(),
                    tag("string", text(format!("observed-{iteration}"))),
                );
            }
            (
                "replace",
                object([
                    ("table", text("articles")),
                    ("id", text(id("article", self.n - 1))),
                    ("fields", fields),
                ]),
            )
        } else {
            let mut fields = article(self.n);
            if let Json::Object(fields) = &mut fields {
                fields.insert(
                    "author_id".into(),
                    tag("reference", text(id("author", self.n / 10 - 1))),
                );
            }
            (
                "insert",
                object([
                    ("table", text("articles")),
                    ("fields", fields),
                    ("proposed_id", text("benchmark:observation")),
                ]),
            )
        };
        let start = self.start();
        self.call(op, args)?;
        let durable = Instant::now();
        let events = self.wait_events(start, durable)?;
        let mut sample = self.report(start)?;
        let mut seen = std::collections::BTreeSet::new();
        for (event, _, _) in &events {
            let event = map(event)?;
            check(event["kind"] == text("data"), "unexpected callback kind")?;
            let token = &event["subscription"];
            let (index, (_, q)) = self
                .observers
                .iter()
                .enumerate()
                .find(|(_, v)| &v.0 == token)
                .ok_or("foreign callback")?;
            check(seen.insert(index), "duplicate listener callback")?;
            let value = map(&event["value"])?;
            if self.mode == "count" {
                let lower = map(&list(&map(q)?["predicates"])?[0])?["values"].clone();
                let lower = map(&list(&lower)?[0])?["value"].clone();
                let Json::String(lower) = lower else {
                    return Err("count lower bound".into());
                };
                check(
                    value["kind"] == text("count")
                        && value["count"]
                            == text(
                                self.n + 1
                                    - lower.parse::<usize>().map_err(|_| "count lower bound")?,
                            ),
                    "count callback value",
                )?;
            } else {
                let expected = self.call("fetch", object([("query", q.clone())]))?;
                check(
                    value["kind"] == text("rows") && value["rows"] == expected,
                    "row callback value",
                )?;
            }
        }
        let Json::Object(fields) = &mut sample else {
            return Err("sample shape".into());
        };
        let first = events.iter().map(|v| v.1).min().ok_or("missing callback")?;
        let last = events.iter().map(|v| v.1).max().ok_or("missing callback")?;
        fields.insert("dispatch_complete_ns".into(), fields["value"].clone());
        fields.insert("value".into(), Json::Integer(last as i64));
        fields.insert("first_callback_ns".into(), Json::Integer(first as i64));
        fields.insert("last_callback_ns".into(), Json::Integer(last as i64));
        fields.insert(
            "return_to_first_callback_ns".into(),
            Json::Integer(events.iter().map(|v| v.2).min().ok_or("missing callback")? as i64),
        );
        fields.insert(
            "return_to_last_callback_ns".into(),
            Json::Integer(events.iter().map(|v| v.2).max().ok_or("missing callback")? as i64),
        );
        fields.insert("callback_count".into(), Json::Integer(events.len() as i64));
        if self.mode == "columns" {
            self.call(
                "replace",
                object([
                    ("table", text("articles")),
                    ("id", text(id("article", self.n - 1))),
                    ("fields", article(self.n - 1)),
                ]),
            )?;
        } else {
            self.call(
                "remove",
                object([
                    ("table", text("articles")),
                    ("id", text("benchmark:observation")),
                ]),
            )?;
        }
        let now = Instant::now();
        self.wait_events(now, now)?;
        Ok(sample)
    }
    fn boundary_sample(&mut self) -> Result<Json> {
        let mut fields = article(0);
        if let Json::Object(fields) = &mut fields {
            fields.insert("score".into(), tag("integer", text(i64::MAX)));
            fields.insert("at".into(), tag("timestamp", text(i64::MIN)));
            fields.insert("ratio".into(), tag("float", text("42.0")));
            fields.insert("title".into(), tag("string", text("東京🌿\0extrema")));
            fields.insert("note".into(), object([("kind", text("null"))]));
        }
        let probe = self.call(
            "insert",
            object([
                ("table", text("articles")),
                ("fields", fields),
                ("proposed_id", text("benchmark:extrema")),
            ]),
        )?;
        check(
            map(&map(&probe)?["fields"])?["ratio"] == tag("float", text("42")),
            "integral float kind",
        )?;
        let start = self.start();
        let schema = self.call("schema", object([]))?;
        let protection = self.call("protection", object([]))?;
        let fetched = self.call(
            "fetch",
            object([(
                "query",
                query(
                    "articles",
                    vec![predicate(
                        "id",
                        "Eq",
                        tag("string", text("benchmark:extrema")),
                    )],
                    None,
                    1,
                ),
            )]),
        )?;
        let sample = self.report(start)?;
        check(
            map(&schema)?["version"] == text(1)
                && protection == self.protection
                && fetched == Json::Array(vec![probe]),
            "boundary result",
        )?;
        self.call(
            "remove",
            object([
                ("table", text("articles")),
                ("id", text("benchmark:extrema")),
            ]),
        )?;
        Ok(sample)
    }
    fn report(&self, start: Instant) -> Result<Json> {
        let elapsed = i64::try_from(start.elapsed().as_nanos()).map_err(|_| "duration overflow")?;
        Ok(object([
            ("value", Json::Integer(elapsed)),
            ("status", text("ok")),
            ("correct", Json::Bool(true)),
            ("input_bytes", Json::Integer(self.input_bytes as i64)),
            ("output_bytes", Json::Integer(self.output_bytes as i64)),
        ]))
    }
    fn start(&mut self) -> Instant {
        self.input_bytes = 0;
        self.output_bytes = 0;
        Instant::now()
    }
    fn context() -> Json {
        object([
            ("identity", object([("kind", text("null"))])),
            ("now_ms", text(1000)),
        ])
    }
    fn enqueue(&mut self, changed: Option<usize>) -> Result<Json> {
        let request = self.call("request", object([]))?;
        let admitted = self.call(
            "begin",
            object([
                ("request", request.clone()),
                (
                    "definition",
                    object([
                        ("name", text("benchmark-v1")),
                        ("arguments", object([])),
                        ("reference_paths", Json::Array(vec![])),
                    ]),
                ),
                ("value", tag("object", object([]))),
                ("context", Self::context()),
            ]),
        )?;
        check(
            admitted == Json::Bool(true),
            "unexpected admission contention",
        )?;
        if let Some(iteration) = changed {
            cooperative_turn();
            let mut fields = article(0);
            if let Json::Object(fields) = &mut fields {
                fields.insert(
                    "title".into(),
                    tag("string", text(format!("command-{iteration}"))),
                );
            }
            self.call(
                "scope_replace",
                object([
                    ("request", request.clone()),
                    ("table", text("articles")),
                    ("id", text(id("article", 0))),
                    ("fields", fields),
                ]),
            )?;
        }
        let result = self.call(
            "finish",
            object([
                ("request", request),
                ("result", object([("kind", text("null"))])),
            ]),
        )?;
        Ok(map(&result)?["key"].clone())
    }
    pub fn verify_retry(&mut self, count: usize) -> Result<Json> {
        check(
            [10, 100].contains(&count) && self.queue == 0,
            "invalid retry fixture",
        )?;
        let mut keys = Vec::new();
        for _ in 0..count {
            keys.push(self.enqueue(None)?);
        }
        let status = self.call("queue_status", object([]))?;
        let entries = list(&map(&status)?["commands"])?;
        check(entries.len() == count, "queued retry cardinality")?;
        for entry in entries {
            let entry = map(entry)?;
            check(
                entry["state"] == text("queued")
                    && entry["attempts"] == text(0)
                    && entry["next_due_ms"] == Json::Null,
                "initial retry status",
            )?;
        }
        let first = self.call(
            "prepare_push",
            object([("now_ms", text(1000)), ("jitter_basis_points", text(0))]),
        )?;
        check(
            list(&map(&first)?["envelopes"])?.len() == count
                && map(&first)?["next_due_ms"] == text(2000),
            "first retry deadline",
        )?;
        let early = self.call(
            "prepare_push",
            object([("now_ms", text(1999)), ("jitter_basis_points", text(0))]),
        )?;
        check(
            list(&map(&early)?["envelopes"])?.is_empty(),
            "retry before persisted deadline",
        )?;
        let second = self.call(
            "prepare_push",
            object([("now_ms", text(2000)), ("jitter_basis_points", text(0))]),
        )?;
        check(
            map(&first)?["envelopes"] == map(&second)?["envelopes"]
                && map(&second)?["next_due_ms"] == text(4000),
            "retry frozen bytes/backoff",
        )?;
        let status = self.call("queue_status", object([]))?;
        for entry in list(&map(&status)?["commands"])? {
            let entry = map(entry)?;
            check(
                entry["state"] == text("sent")
                    && entry["attempts"] == text(2)
                    && entry["next_due_ms"] == text(4000),
                "persisted retry metadata",
            )?;
        }
        self.call(
            "receive",
            object([("key", keys[0].clone()), ("outcome", Json::Null)]),
        )?;
        let status = self.call("queue_status", object([]))?;
        let entry = list(&map(&status)?["commands"])?
            .iter()
            .find(|entry| map(entry).is_ok_and(|entry| entry["key"] == keys[0]))
            .ok_or("acknowledged retry entry")?;
        let entry = map(entry)?;
        check(
            entry["state"] == text("acknowledged")
                && entry["attempts"] == text(2)
                && entry["next_due_ms"] == Json::Null,
            "ack retry stopped",
        )?;
        Ok(object([
            ("count", text(count)),
            ("attempts", text(2)),
            ("next_due_ms", text(4000)),
            ("frozen_equal", Json::Bool(true)),
            ("clock", text("injected correctness only")),
        ]))
    }
    pub fn set_queue(&mut self, count: usize) -> Result<()> {
        check(
            [0, 10, 100].contains(&count) && self.queue == 0,
            "invalid baseline queue",
        )?;
        for _ in 0..count {
            self.enqueue(None)?;
        }
        if count > 0 {
            self.call(
                "prepare_push",
                object([("now_ms", text(1000)), ("jitter_basis_points", text(0))]),
            )?;
        }
        self.queue = count;
        self.baseline_status = self.call("queue_status", object([]))?;
        Ok(())
    }
    fn command_sample(&mut self, iteration: usize) -> Result<Json> {
        if self.baseline_status == Json::Null {
            self.baseline_status = self.call("queue_status", object([]))?;
        }
        let start = self.start();
        let key = self.enqueue(Some(iteration))?;
        let mut sample = self.report(start)?;
        if let Json::Object(fields) = &mut sample {
            fields.insert("created_command_key".into(), key.clone());
        }
        let status = self.call("queue_status", object([]))?;
        check(
            list(&map(&status)?["commands"])?.len() == self.queue + 1,
            "command queue count",
        )?;
        self.call(
            "prepare_push",
            object([("now_ms", text(1000)), ("jitter_basis_points", text(0))]),
        )?;
        self.call(
            "receive",
            object([
                ("key", key.clone()),
                (
                    "outcome",
                    object([
                        ("status", text("applied")),
                        ("result", object([("kind", text("null"))])),
                    ]),
                ),
            ]),
        )?;
        self.call(
            "reconcile",
            object([
                ("key", key.clone()),
                (
                    "plan",
                    object([("writes", Json::Array(vec![])), ("mappings", object([]))]),
                ),
            ]),
        )?;
        self.call("forget", object([("key", key)]))?;
        self.call(
            "replace",
            object([
                ("table", text("articles")),
                ("id", text(id("article", 0))),
                ("fields", article(0)),
            ]),
        )?;
        let status = self.call("queue_status", object([]))?;
        check(
            status == self.baseline_status,
            "exact command queue restoration",
        )?;
        Ok(sample)
    }
    fn configure(&mut self) -> Result<()> {
        if !self.configured {
            self.call(
                "entity_configure",
                object([(
                    "config",
                    object([
                        ("scope", text("benchmark")),
                        ("tables", Json::Array(vec![text("articles")])),
                    ]),
                )]),
            )?;
            self.configured = true;
        }
        Ok(())
    }
    fn entity_sample(&mut self, iteration: usize) -> Result<Json> {
        self.configure()?;
        let start = self.start();
        let row = self.call(
            "patch",
            object([
                ("table", text("articles")),
                ("id", text(id("article", 0))),
                (
                    "fields",
                    object([("title", tag("string", text(format!("entity-{iteration}"))))]),
                ),
                ("context", Self::context()),
            ]),
        )?;
        let sample = self.report(start)?;
        let mut expected = article(0);
        let Json::Object(ref mut fields) = expected else {
            return Err("article object".into());
        };
        fields.insert(
            "title".into(),
            tag("string", text(format!("entity-{iteration}"))),
        );
        check(
            row == object([("id", text(id("article", 0))), ("fields", expected)]),
            "complete patch result",
        )?;
        let status = self.call("queue_status", object([]))?;
        let entries = list(&map(&status)?["commands"])?;
        check(entries.len() == 1, "entity queue count")?;
        let key = map(&entries[0])?["key"].clone();
        self.call("cancel", object([("key", key.clone())]))?;
        let status = self.call("queue_status", object([]))?;
        if list(&map(&status)?["commands"])?
            .iter()
            .any(|v| map(v).is_ok_and(|v| v["key"] == key))
        {
            self.call("forget", object([("key", key)]))?;
        }
        Ok(sample)
    }
    fn change(index: usize, title: Option<String>, clock: i64, full: bool) -> Result<Json> {
        let fields = if full {
            article(index)
        } else {
            object([(
                "title",
                tag("string", text(title.ok_or("missing pull title")?)),
            )])
        };
        let times = Json::Object(
            map(&fields)?
                .keys()
                .map(|name| (name.clone(), text(clock)))
                .collect(),
        );
        Ok(object([
            ("table", text("articles")),
            ("record_id", text(id("article", index))),
            (
                "op",
                text(if full {
                    "create"
                } else {
                    "update"
                }),
            ),
            ("changes", fields),
            ("removed_fields", Json::Array(vec![])),
            ("timestamp_ms", text(clock)),
            ("field_timestamps", times),
        ]))
    }
    fn page(&mut self, changes: Vec<Json>, checkpoint: String, complete: bool) -> Result<()> {
        let batch = self.call("begin_pull", object([]))?;
        let request = map(&batch)?["request"].clone();
        self.call(
            "apply_pull",
            object([
                ("request", request),
                (
                    "page",
                    object([
                        ("checkpoint", text(checkpoint)),
                        ("complete", Json::Bool(complete)),
                        ("changes", Json::Array(changes)),
                    ]),
                ),
            ]),
        )?;
        Ok(())
    }
    fn pull_sample(&mut self, iteration: usize) -> Result<Json> {
        self.configure()?;
        self.clock += 2;
        let clock = self.clock;
        let changes = (0..100)
            .map(|index| {
                Self::change(
                    index,
                    Some(format!("pulled-{iteration}-{index}")),
                    clock,
                    false,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let start = self.start();
        self.page(changes, format!("measured-{iteration}-{clock}"), true)?;
        let sample = self.report(start)?;
        let changed = self.call(
            "fetch",
            object([(
                "query",
                query(
                    "articles",
                    vec![predicate("score", "Lt", tag("integer", text(100)))],
                    Some(("score", "Asc")),
                    100,
                ),
            )]),
        )?;
        let expected = Json::Array(
            (0..100)
                .map(|index| {
                    let mut fields = article(index);
                    if let Json::Object(fields) = &mut fields {
                        fields.insert(
                            "title".into(),
                            tag("string", text(format!("pulled-{iteration}-{index}"))),
                        );
                    }
                    object([("id", text(id("article", index))), ("fields", fields)])
                })
                .collect(),
        );
        check(changed == expected, "pull must change exact hundred rows")?;
        self.page(
            (0..100)
                .map(|index| Self::change(index, Some(format!("東京🌿-{index}")), clock + 1, false))
                .collect::<Result<Vec<_>>>()?,
            format!("restored-{iteration}-{clock}"),
            true,
        )?;
        Ok(sample)
    }
    fn bootstrap(&mut self) -> Result<()> {
        self.clock += 2;
        self.epoch += 1;
        let clock = self.clock;
        for first in (0..self.n).step_by(100) {
            let end = (first + 100).min(self.n);
            self.page(
                (first..end)
                    .map(|index| Self::change(index, None, clock, true))
                    .collect::<Result<Vec<_>>>()?,
                format!("bootstrap-{}-{first}", self.epoch),
                end == self.n,
            )?;
        }
        Ok(())
    }
    fn reset_sample(&mut self) -> Result<Json> {
        self.configure()?;
        if self.epoch == 0 {
            self.bootstrap()?;
        }
        let start = self.start();
        self.call("reset", object([("mode", text("preserve_recovery"))]))?;
        let sample = self.report(start)?;
        let progress = self.call("sync_progress", object([]))?;
        check(map(&progress)?["pages"] == text(0), "reset progress")?;
        for table in ["articles", "authors", "tags", "article_tags"] {
            let rows = self.call("fetch", object([("query", query(table, vec![], None, 1))]))?;
            check(list(&rows)?.is_empty(), "reset must clear actual rows")?;
        }
        self.seed(
            (0..self.n / 10)
                .map(|index| {
                    insert(
                        "authors",
                        id("author", index),
                        object([("name", tag("string", text(format!("author-{index:06}"))))]),
                    )
                })
                .collect(),
        )?;
        self.bootstrap()?;
        Ok(sample)
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.call("close", object([]));
    }
}
// One real Pending/resume boundary in the caller; no core borrow spans it.
fn cooperative_turn() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    struct Turn(bool);
    impl Future for Turn {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
    let mut future = std::pin::pin!(Turn(false));
    let mut cx = Context::from_waker(Waker::noop());
    while future.as_mut().poll(&mut cx).is_pending() {
        std::thread::yield_now();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    static OWNERS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test]
    fn p01_p02_literal_fixture_real_public_operations_and_protection() {
        let _owners = OWNERS.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut harness = Harness::new(100, None).unwrap();
        harness.verify().unwrap();
        let mut proof = Vec::new();
        harness
            .verify_to(|table, row| {
                proof.push((table.to_owned(), row.clone()));
                Ok(())
            })
            .unwrap();
        assert_eq!(proof.len(), 112);
        assert_eq!(proof[0], ("articles".into(), row(0)));
        assert_eq!(proof[100].0, "authors");
        assert_eq!(proof[110], ("tags".into(), Json::Null));
        assert_eq!(proof[111], ("article_tags".into(), Json::Null));
        for operation in [
            "point",
            "selective",
            "sorted-related",
            "batch-insert",
            "batch-update",
            "batch-remove",
            "optimistic-command",
            "entity-patch",
            "non-noop-pull",
            "reset",
        ] {
            let sample = harness.sample(operation, 0).unwrap();
            let Json::Object(sample) = sample else {
                panic!("sample object")
            };
            assert_eq!(sample["correct"], Json::Bool(true));
            assert_eq!(sample["status"], Json::String("ok".into()));
        }
        harness.verify().unwrap();
    }
    #[test]
    fn p01_p02_protected_engine_uses_identical_typed_operations() {
        let _owners = OWNERS.lock().unwrap_or_else(|poison| poison.into_inner());
        let directory =
            std::env::temp_dir().join(format!("hoya-benchmark-tests-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let directory = directory.canonicalize().unwrap();
        let mut harness = Harness::new(100, Some(&directory)).unwrap();
        assert_eq!(
            map(&harness.protection).unwrap()["cipher"],
            text("SQLCipher")
        );
        assert_eq!(map(&harness.protection).unwrap()["version"], text("4.19.0"));
        harness.verify().unwrap();
        for operation in [
            "point",
            "selective",
            "sorted-related",
            "batch-insert",
            "batch-update",
            "batch-remove",
            "optimistic-command",
            "entity-patch",
            "non-noop-pull",
            "reset",
        ] {
            harness.sample(operation, 0).unwrap();
        }
        harness.verify().unwrap();
    }
    #[test]
    fn p03_rss_cycles_are_real_owned_owners_with_full_first_and_final_proof() {
        let _owners = OWNERS.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut checkpoints = Vec::new();
        let mut phases = Vec::new();
        let mut rows = 0;
        let protection = rss_cycles(None, |event| {
            match event {
                ResourceEvent::Checkpoint(phase, cycle) => checkpoints.push((phase, cycle)),
                ResourceEvent::ProofBegin(phase) => phases.push((phase, "begin")),
                ResourceEvent::ProofEnd(phase) => phases.push((phase, "end")),
                ResourceEvent::ProofRow(_, _) => rows += 1,
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(checkpoints.len(), 61);
        assert_eq!(checkpoints[0], ("baseline", None));
        for cycle in 0..30 {
            assert_eq!(checkpoints[cycle * 2 + 1], ("after_open", Some(cycle)));
            assert_eq!(checkpoints[cycle * 2 + 2], ("after_close", Some(cycle)));
        }
        assert_eq!(
            phases,
            [
                ("initial", "begin"),
                ("initial", "end"),
                ("final", "begin"),
                ("final", "end")
            ]
        );
        assert_eq!(rows, 224);
        assert_eq!(map(&protection).unwrap()["encrypted"], Json::Bool(false));
        // Every owner really closed: the global admission coordinator is idle.
        let mut fresh = Harness::cold(0, None, false).unwrap();
        fresh.verify().unwrap();
    }
    #[test]
    fn p01_wrong_batch_change_ids_or_before_after_never_count_as_correct() {
        let result = Json::Array(
            (0..10)
                .map(|index| {
                    object([
                        ("table", text("articles")),
                        ("id", text(id("article", index))),
                        ("before", row(index)),
                        ("after", Json::Null),
                    ])
                })
                .collect(),
        );
        verify_batch_result("batch-remove", 100, 0, &result).unwrap();
        let mut wrong = result.clone();
        {
            let Json::Array(ref mut changes) = wrong else {
                panic!("array")
            };
            let Json::Object(ref mut first) = changes[0] else {
                panic!("change")
            };
            first.insert("before".into(), row(99));
        }
        assert!(verify_batch_result("batch-remove", 100, 0, &wrong).is_err());
        {
            let Json::Array(ref mut changes) = wrong else {
                panic!("array")
            };
            let Json::Object(ref mut first) = changes[0] else {
                panic!("change")
            };
            first.insert("before".into(), row(0));
            first.insert("id".into(), text("wrong-id"));
        }
        assert!(verify_batch_result("batch-remove", 100, 0, &wrong).is_err());
    }
    #[test]
    fn p02_real_queue_retry_backoff_and_distinct_callback_fanout() {
        let _owners = OWNERS.lock().unwrap_or_else(|poison| poison.into_inner());
        for count in [10, 100] {
            let mut harness = Harness::new(100, None).unwrap();
            let retry = harness.verify_retry(count).unwrap();
            assert_eq!(map(&retry).unwrap()["attempts"], text(2));
            assert_eq!(map(&retry).unwrap()["next_due_ms"], text(4000));
            assert_eq!(map(&retry).unwrap()["frozen_equal"], Json::Bool(true));
            harness.verify().unwrap();
        }
        for count in [10, 100] {
            let mut harness = Harness::new(100, None).unwrap();
            harness.set_queue(count).unwrap();
            harness.sample("optimistic-command", 0).unwrap();
            harness.verify().unwrap();
        }
        let mut harness = Harness::new(100, None).unwrap();
        for fanout in [10, 100] {
            for mode in ["membership", "columns", "count"] {
                harness.set_observers(fanout, "distinct", mode).unwrap();
                let sample = harness.sample("reactivity", 0).unwrap();
                assert_eq!(
                    map(&sample).unwrap()["callback_count"],
                    Json::Integer(fanout as i64)
                );
            }
        }
        harness
            .set_observers(0, "equivalent", "membership")
            .unwrap();
        harness.verify().unwrap();
    }
    #[test]
    fn p02_cold_empty_create_same_key_reopen_and_authored_upgrade() {
        let _owners = OWNERS.lock().unwrap_or_else(|poison| poison.into_inner());
        {
            let mut empty = Harness::cold(0, None, false).unwrap();
            empty.verify().unwrap();
        }
        let directory =
            std::env::temp_dir().join(format!("hoya-benchmark-cold-tests-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let directory = directory.canonicalize().unwrap();
        {
            let mut empty = Harness::cold(0, Some(&directory), false).unwrap();
            empty.verify().unwrap();
        }
        {
            let mut seeded = Harness::new(100, Some(&directory)).unwrap();
            seeded.verify().unwrap();
        }
        {
            let mut reopened = Harness::cold(100, Some(&directory), false).unwrap();
            reopened.verify().unwrap();
        }
        {
            let mut upgraded = Harness::cold(100, Some(&directory), true).unwrap();
            upgraded.verify().unwrap();
            let schema = upgraded.call("schema", object([])).unwrap();
            assert_eq!(map(&schema).unwrap()["version"], text("2"));
            assert_eq!(map(&map(&map(&map(&schema).unwrap()["tables"]).unwrap()["articles"]).unwrap()["columns"]).unwrap()["benchmark_extra"],object([("kind",text("String")),("optional",Json::Bool(true))]));
        }
    }
    #[test]
    fn p02_observations_and_boundary_are_real_decoded_callbacks() {
        let _owners = OWNERS.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut harness = Harness::new(100, None).unwrap();
        for mode in ["membership", "columns", "count"] {
            harness.set_observers(1, "equivalent", mode).unwrap();
            let sample = harness.sample("reactivity", 0).unwrap();
            check(
                map(&sample).unwrap()["correct"] == Json::Bool(true),
                "callback result",
            )
            .unwrap();
        }
        harness
            .set_observers(0, "equivalent", "membership")
            .unwrap();
        let sample = harness.sample("boundary-known", 0).unwrap();
        assert_eq!(map(&sample).unwrap()["correct"], Json::Bool(true));
        harness.verify().unwrap();
    }
}
