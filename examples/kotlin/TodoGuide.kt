// A Todo app with the HoyaDB Android (Kotlin) binding. The documentation build compiles this file
// with the binding and runs it on an Android emulator against the real native core; the named
// regions are the Kotlin getting-started snippets. `main` starts the guide from a plain coroutine
// so the example needs no coroutine library; in an app, call todoGuide from your own coroutine
// scope and pass an executor that posts to the main thread.
import kotlin.coroutines.startCoroutine
// docs:start schema
import dev.hoyadb.*
import dev.hoyadb.testing.FakeHandler
import dev.hoyadb.testing.FakeTransport
import java.util.concurrent.Executor

val schema = Schema(
    version = 1,
    tables = mapOf(
        "todos" to Table(columns = mapOf(
            "title" to Column(ColumnKind.String),
            "done" to Column(ColumnKind.Boolean),
        )),
    ),
)
// docs:end schema

/** Observation callbacks run on the executor; this records what they saw, thread-safely. */
class Seen {
    private val titles = java.util.concurrent.atomic.AtomicReference(listOf<String>())
    fun record(rows: List<Row>) = titles.set(rows.mapNotNull { (it.fields["title"] as? Cell.StringValue)?.value }.sorted())
    val current: List<String> get() = titles.get()
}

/** Waits without blocking a binding thread: each retry suspends and resumes on a separate ticker. */
val ticker = java.util.concurrent.Executors.newSingleThreadScheduledExecutor { work -> Thread(work, "guide-ticker").apply { isDaemon = true } }
suspend fun until(message: String, predicate: () -> Boolean) {
    repeat(2000) {
        if (predicate()) return
        kotlin.coroutines.suspendCoroutine<Unit> { next ->
            ticker.schedule({ next.resumeWith(Result.success(Unit)) }, 1, java.util.concurrent.TimeUnit.MILLISECONDS)
        }
    }
    error("timed out waiting for $message")
}

suspend fun todoGuide(ui: Executor) {
    // docs:start open
    val db = Database(ui)
    db.openMemory(schema)
    // docs:end open
    try {
        // docs:start query
        val openTasks = Query("todos", predicates = listOf(Predicate("done", Operator.Eq, listOf(Cell.BooleanValue(false)))),
            order = Order("title"))
        val rows: List<Row> = db.fetch(openTasks)
        // Each Row has an id and typed fields, such as fields["title"] == Cell.StringValue("Buy milk").
        // docs:end query
        check(rows.isEmpty()) { "a new database has no tasks" }

        // docs:start observe
        val seen = Seen()
        val subscription = db.observe(openTasks) { value ->
            if (value is ObservedValue.Rows) seen.record(value.rows)
        }
        // The callback first receives the current rows on `ui`, then again after each change.
        // docs:end observe

        // docs:start command-definition
        val addTaskV1 = CommandDefinition("add-task-v1",
            arguments = mapOf("title" to CommandDefinition.Argument(CommandDefinition.Kind.String)))
        // The local handler runs on the device. Its scope is a view for this command only, and all of
        // its writes commit together when it returns. New rows get temporary "local:" IDs.
        val addTask: suspend (CommandScope) -> JSONValue = { scope ->
            val args = scope.arguments() as JSONValue.ObjectValue
            val title = args.value["title"] as JSONValue.StringValue
            val task = scope.insert("todos", mapOf("title" to Cell.StringValue(title.value), "done" to Cell.BooleanValue(false)))
            JSONValue.ObjectValue(mapOf("id" to JSONValue.StringValue(task.id)))
        }
        // docs:end command-definition

        // docs:start execute
        val added = db.command(addTaskV1, JSONValue.ObjectValue(mapOf("title" to JSONValue.StringValue("Buy milk"))),
            CapturedContext(JSONValue.StringValue("ada"), 1000), addTask)
        val localId = ((added.value as JSONValue.ObjectValue).value["id"] as JSONValue.StringValue).value
        // localId starts with "local:"; added.key identifies the queued command.
        // docs:end execute
        check(localId.startsWith("local:")) { "the new task has a temporary local ID, got $localId" }
        until("the observer sees the new task") { seen.current == listOf("Buy milk") }
        check(db.queueStatus().commands.single().state == QueueState.Queued) { "the command is queued for sync" }

        // docs:start backend
        // Your server runs its own version of add-task-v1 and answers with the task's real ID. In a real
        // app, send the envelopes to your HTTP API; here the test backend plays the server.
        val serverAddTask: FakeHandler = { RemoteOutcome.Applied(JSONValue.ObjectValue(mapOf("id" to JSONValue.StringValue("server-task-1")))) }
        val backend = FakeTransport(commandHandlers = mapOf("add-task-v1" to serverAddTask))
        // docs:end backend

        // docs:start sync
        val batch = db.preparePush(1000, 0)
        val answers = backend.push(batch.envelopes.joinToString(",", "[", "]"))
        db.receiveWire(answers)
        // Confirm the command: map the one local ID it created to the ID the server assigned.
        for (pending in db.pendingReconciliations()) {
            if (pending.key == added.key) db.reconcile(pending.key, ReconcilePlan(emptyList(), mapOf(localId to "server-task-1")))
        }
        // docs:end sync
        check(db.queueStatus().commands.single().state == QueueState.Applied) { "the command was confirmed" }
        val synced = db.fetch(openTasks).map { it.id }
        check(synced == listOf("server-task-1")) { "the task now has the server ID, got $synced" }

        // docs:start close
        subscription.close()
        // docs:end close
    } finally {
        db.close()
    }
}

fun main() {
    val ui = java.util.concurrent.Executors.newSingleThreadExecutor()
    val finished = java.util.concurrent.CountDownLatch(1)
    var outcome: Result<Unit>? = null
    val guide: suspend () -> Unit = { todoGuide(ui) }
    guide.startCoroutine(kotlin.coroutines.Continuation(kotlin.coroutines.EmptyCoroutineContext) { result ->
        outcome = result
        finished.countDown()
    })
    check(finished.await(120, java.util.concurrent.TimeUnit.SECONDS)) { "the guide did not finish" }
    ui.shutdown()
    outcome!!.getOrThrow()
    println("kotlin tour: ok")
}
