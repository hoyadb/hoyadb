package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.Executors

object QueueTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        test("K09_K10_exact_queue_retry_receipt_outcome_and_managed_sent_event") {
            val ui = Executors.newSingleThreadExecutor { Thread(it, "queue-selected-ui") }; val db = Database(ui)
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val command = db.command(CommandDefinition("queue-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 0)) { JSONValue.IntegerValue(Long.MAX_VALUE) }
                check(db.queueStatus().commands.single().state == QueueState.Queued)
                val initial = CountDownLatch(1); val sent = CountDownLatch(1); val applied = CountDownLatch(1)
                val observer = db.observeOutcome(command.key) { outcome ->
                    check(Thread.currentThread().name == "queue-selected-ui")
                    when (outcome) { CommandOutcome.Queued -> initial.countDown(); CommandOutcome.Sent -> sent.countDown(); is CommandOutcome.Applied -> { check(outcome.result == JSONValue.IntegerValue(Long.MAX_VALUE)); applied.countDown() }; else -> {} }
                }
                check(initial.await(30, TimeUnit.SECONDS))
                val first = db.preparePush(0, 0); check(first.envelopes.size == 1 && first.nextDueMs == 1000L)
                check(sent.await(30, TimeUnit.SECONDS))
                var status = db.queueStatus().commands.single(); check(status.attempts == 1L && status.nextDueMs == 1000L)
                check(db.preparePush(999, 0).envelopes.isEmpty())
                val retry = db.preparePush(1000, 0); check(retry.envelopes == first.envelopes)
                status = db.queueStatus().commands.single(); check(status.attempts == 2L && status.nextDueMs == 3000L)
                db.receive(command.key, null); check(db.queueStatus().commands.single().state == QueueState.Acknowledged)
                check(db.preparePush(999999, 0).envelopes.isEmpty())
                db.receive(command.key, RemoteOutcome.Applied(JSONValue.IntegerValue(Long.MAX_VALUE)))
                check(db.pendingReconciliations().single().key == command.key)
                db.reconcile(command.key, ReconcilePlan(emptyList())); check(applied.await(30, TimeUnit.SECONDS))
                val d = db.diagnostics(); check(d.live.value == "0" && d.schemaVersion == 1L)
                observer.close()
            } finally { db.close(); ui.shutdownNow() }
        }
        test("K09_fake_core_JSON_helpers_preserve_i64_without_platform_numeric_parse") {
            val value = JSONValue.decodeJSON("{\"large\":9223372036854775807,\"minimum\":-9223372036854775808,\"float\":42.0}") as JSONValue.ObjectValue
            check(value.value.getValue("large") == JSONValue.IntegerValue(Long.MAX_VALUE)); check(value.value.getValue("minimum") == JSONValue.IntegerValue(Long.MIN_VALUE))
            check(value.value.getValue("float") is JSONValue.FloatValue)
            val roundTrip = JSONValue.decodeJSON(value.encodeJSON()); check(roundTrip == value)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
