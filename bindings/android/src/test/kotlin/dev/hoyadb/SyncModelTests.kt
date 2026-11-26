// API-0011 / TC-0036 K09–K12: independent literal wire shapes and immutable ownership.
package dev.hoyadb

object SyncModelTests {
    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun run(name: String, body: () -> Unit) {
            try { body(); println("PASS $name") }
            catch (error: Throwable) { failures++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) }
        }
        fun rejects(body: () -> Unit) {
            try { body(); error("expected rejection") }
            catch (error: HoyaException) { check(error.kind == "InvalidResponse") }
        }
        fun entityWire(operation: String = "update"): ProtocolValue = obj(
            "table" to str("articles"), "record_id" to str("a"), "op" to str(operation),
            "changes" to obj("score" to obj("kind" to str("integer"), "value" to str("9223372036854775807"))),
            "removed_fields" to ProtocolValue.ArrayValue(listOf(str("note"))), "timestamp_ms" to str("2000"),
            "field_timestamps" to obj("score" to str("1999"), "note" to str("2000")))
        run("K11_entity_config_and_page_are_exact_owned_structures") {
            val fields = mutableMapOf<String, Cell>("score" to Cell.IntegerValue(Long.MAX_VALUE))
            val removed = mutableListOf("note")
            val timestamps = mutableMapOf("score" to 1999L, "note" to 2000L)
            val change = EntityChange("articles", "a", EntityOperation.Update, fields, removed, 2000, timestamps)
            val changes = mutableListOf(change)
            val page = PullPage("cp🌿", true, changes)
            val tables = mutableListOf("articles"); val config = EntityConfig("membership", tables)
            fields.clear(); removed.clear(); timestamps.clear(); changes.clear(); tables.clear()
            check(change.toProtocolValue() == entityWire())
            check(config.toProtocolValue() == obj("scope" to str("membership"), "tables" to ProtocolValue.ArrayValue(listOf(str("articles")))))
            check(page.toProtocolValue() == obj("checkpoint" to str("cp🌿"), "complete" to ProtocolValue.BooleanValue(true), "changes" to ProtocolValue.ArrayValue(listOf(entityWire()))))
            val decoded = PullPage.fromProtocolValue(page.toProtocolValue())
            check(decoded.checkpoint == "cp🌿" && decoded.complete && decoded.changes.single().recordId == "a")
            check(decoded.changes.single().changes["score"] == Cell.IntegerValue(Long.MAX_VALUE))
            check(decoded.changes.single().fieldTimestamps["note"] == 2000L)
            for (operation in listOf("create", "update", "delete")) check(EntityChange.fromProtocolValue(entityWire(operation)).toProtocolValue() == entityWire(operation))
            rejects { EntityChange.fromProtocolValue(entityWire("upsert")) }
        }
        run("K09_outcomes_and_all_plan_variants_keep_tagged_extrema_and_exact_keys") {
            val result = JSONValue.IntegerValue(Long.MAX_VALUE)
            val applied = RemoteOutcome.Applied(result)
            check(applied.toProtocolValue() == obj("status" to str("applied"), "result" to obj("kind" to str("integer"), "value" to str("9223372036854775807"))))
            check((RemoteOutcome.fromProtocolValue(applied.toProtocolValue()) as RemoteOutcome.Applied).result == result)
            check(RemoteOutcome.Corrected(JSONValue.Null).toProtocolValue() == obj("status" to str("corrected"), "result" to obj("kind" to str("null"))))
            check(RemoteOutcome.Rejected("business rejection").toProtocolValue() == obj("status" to str("rejected"), "reason" to str("business rejection")))
            val fields = mutableMapOf<String, Cell>("score" to Cell.IntegerValue(Long.MIN_VALUE))
            val writes = mutableListOf<PlanWrite>(PlanWrite.Put("articles", "a", fields), PlanWrite.Remove("articles", "b"), PlanWrite.Entity(EntityChange.fromProtocolValue(entityWire())))
            val mappings = mutableMapOf("local:a" to "server:a"); val plan = ReconcilePlan(writes, mappings)
            fields.clear(); writes.clear(); mappings.clear()
            val expected = obj("writes" to ProtocolValue.ArrayValue(listOf(
                obj("op" to str("put"), "table" to str("articles"), "id" to str("a"), "fields" to obj("score" to obj("kind" to str("integer"), "value" to str("-9223372036854775808")))),
                obj("op" to str("remove"), "table" to str("articles"), "id" to str("b")), obj("op" to str("entity"), "change" to entityWire()))), "mappings" to obj("local:a" to str("server:a")))
            check(plan.toProtocolValue() == expected)
            val pending = PendingReconciliation.fromProtocolValue(obj("key" to str("k"), "outcome" to applied.toProtocolValue()))
            check(pending.key == "k" && pending.outcome is RemoteOutcome.Applied)
            rejects { RemoteOutcome.fromProtocolValue(obj("status" to str("applied"), "reason" to str("wrong union"))) }
            rejects { PendingReconciliation.fromProtocolValue(obj("key" to str("k"), "outcome" to ProtocolValue.Null)) }
        }
        run("K09_queue_status_preserves_full_unsigned_counts_and_payload_free_shapes") {
            fun entry(state: String, due: ProtocolValue = ProtocolValue.Null) = obj(
                "kind" to str("command"), "table" to ProtocolValue.Null, "record_id" to ProtocolValue.Null, "key" to str("private-key"), "state" to str(state),
                "blocked_on" to ProtocolValue.ArrayValue(listOf(str("dependency"))), "awaiting_local" to ProtocolValue.BooleanValue(false), "awaiting_key" to ProtocolValue.Null, "rolled_back" to ProtocolValue.BooleanValue(false),
                "attempts" to str("4294967295"), "next_due_ms" to due)
            val maximum = "18446744073709551615"
            val status = QueueStatus.fromProtocolValue(obj("pending" to str(maximum), "sending" to str("1"), "awaiting_outcome" to str("0"), "live" to str(maximum), "blocked" to ProtocolValue.BooleanValue(true), "commands" to ProtocolValue.ArrayValue(listOf(entry("sent", str("2000"))))))
            check(status.pending.value == maximum && status.live.value == maximum && status.blocked)
            val sent = status.commands.single(); check(sent.kind == QueueKind.Command && sent.table == null && sent.recordId == null && sent.attempts == 4294967295L && sent.nextDueMs == 2000L)
            check(sent.blockedOn == listOf("dependency"))
            check(!sent.toString().contains("private-key") && !status.toString().contains("dependency"))
            for (state in listOf("queued", "sent", "acknowledged", "applied", "corrected", "rejected", "cancelled")) {
                val due = if (state == "sent") str("2000") else ProtocolValue.Null
                check(QueueEntry.fromProtocolValue(entry(state, due)).nextDueMs == if (state == "sent") 2000L else null)
            }
            val entityFields = entry("queued").fields().toMutableMap()
            entityFields["kind"] = str("entity"); entityFields["table"] = str("articles"); entityFields["record_id"] = str("a")
            val entity = QueueEntry.fromProtocolValue(ProtocolValue.ObjectValue(entityFields))
            check(entity.kind == QueueKind.Entity && entity.table == "articles" && entity.recordId == "a")
            rejects { QueueEntry.fromProtocolValue(entry("unknown")) }
            val extra = entry("queued").fields().toMutableMap(); extra["identity"] = str("forbidden")
            rejects { QueueEntry.fromProtocolValue(ProtocolValue.ObjectValue(extra)) }
            extra.remove("identity"); extra["attempts"] = str("4294967296")
            rejects { QueueEntry.fromProtocolValue(ProtocolValue.ObjectValue(extra)) }
            // SPEC-0038 / TC-0042 SDK1: the exact eleven-field entry.
            check(sent.awaitingKey == null)
            val blocked = entry("queued").fields().toMutableMap(); blocked["awaiting_key"] = str("A")
            check(QueueEntry.fromProtocolValue(ProtocolValue.ObjectValue(blocked)).awaitingKey == "A")
            blocked["awaiting_key"] = ProtocolValue.BooleanValue(false)
            rejects { QueueEntry.fromProtocolValue(ProtocolValue.ObjectValue(blocked)) }
            val tenField = entry("queued").fields().toMutableMap(); tenField.remove("awaiting_key")
            rejects { QueueEntry.fromProtocolValue(ProtocolValue.ObjectValue(tenField)) }
            var immutable = false
            try { (status.commands as MutableList).clear() } catch (_: UnsupportedOperationException) { immutable = true }
            check(immutable)
        }
        run("K10_K12_push_progress_and_pull_request_decode_without_precision_loss") {
            val frozen = "{\"integer\":9223372036854775807}"
            val batch = PushBatch.fromProtocolValue(obj("envelopes" to ProtocolValue.ArrayValue(listOf(str(frozen))), "next_due_ms" to str("253402300799999"), "has_more" to ProtocolValue.BooleanValue(true)))
            check(batch.envelopes == listOf(frozen) && batch.nextDueMs == 253402300799999L && batch.hasMore)
            val request = PullRequest.fromProtocolValue(obj("request" to str("18446744073709551615"), "since" to ProtocolValue.Null, "scope" to str("membership"), "generation" to str("9223372036854775807")))
            check(request.request.value == "18446744073709551615" && request.since == null && request.generation == Long.MAX_VALUE)
            val progress = SyncProgress.fromProtocolValue(obj("phase" to str("bootstrapping"), "ready" to ProtocolValue.BooleanValue(false), "pages" to str("1"), "changes" to str("100"), "tombstones" to str("18446744073709551615")))
            check(progress.phase == SyncPhase.Bootstrapping && !progress.ready && progress.pages == 1L && progress.changes == 100L && progress.tombstones.value == "18446744073709551615")
            rejects { PullRequest.fromProtocolValue(obj("request" to str("0"), "since" to ProtocolValue.Null, "scope" to str("m"), "generation" to str("1"))) }
            rejects { SyncProgress.fromProtocolValue(obj("phase" to str("unknown"), "ready" to ProtocolValue.BooleanValue(false), "pages" to str("0"), "changes" to str("0"), "tombstones" to str("0"))) }
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
