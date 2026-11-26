---
source: https://pouchdb.com/guides/databases.html
source_type: doc-site
fetched: 2026-09-13
---

# PouchDB — Overview

Observed, not accepted — this describes PouchDB, not hoyadb.

## What it is

A JS database with two interchangeable "flavors" behind the same API:
**local** (an embedded database using whatever storage the environment
offers — IndexedDB in most browsers, WebSQL in older ones, LevelDB in
Node.js) and **remote** (a thin client speaking directly to a
CouchDB-protocol server — CouchDB itself, Cloudant, Couchbase, etc.).

## Design goal, stated directly

"The goal of PouchDB is to allow you to seamlessly communicate with one or
the other [local or remote] ... You should not notice many differences
between the two, except that of course the local one is much faster!" —
i.e. the *same* CRUD/query/replicate API works whether you're pointed at
an in-browser local store or a remote server, and replication (see
[sync.md](sync.md)) is what reconciles them.

## Relevance to hoyadb

The "local and remote are the same API, replication reconciles them" model
is the oldest version of an idea several later sources reinvent in
different ways (RxDB's storage-adapter-behind-one-API, WatermelonDB's
adapter, Zero/PowerSync/Electric's "sync an existing DB"). Worth noting
PouchDB predates and directly influenced RxDB (RxDB's earliest releases
used PouchDB itself as a storage adapter) — several ideas already logged
in [../rxdb/overview.md](../rxdb/overview.md) trace back to here.
