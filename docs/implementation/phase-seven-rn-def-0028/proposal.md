# DEF-0028: location and input-schema proposal (for prep re-review; nothing implemented)

This is a proposal only. Nothing in it has been implemented.

## Location concern (needs a decision)

DEF-0028 names `bindings/react-native/android/tools/prepare_repo.py` and
`test_prepare_repo.py`. However, `bindings/react-native/package.json` `"files"`
whitelists the whole `android` directory. The tools would therefore ship inside
the packed `@hoyadb/react-native` npm tarball and its consumers' `node_modules`.
They would also sit inside the Gradle library `sourceDir`, where Gradle ignores
them.

Options:

| Option | Location | Effect |
| --- | --- | --- |
| A | DEF path, plus a negation entry such as `"!android/tools"` in `"files"` | `package.json` and the packed artifact identity change |
| B (recommended) | `bindings/react-native/tools/android/prepare_repo.py` and `test_prepare_repo.py` | not packed, no package change |

Everything below uses `<tooldir>` for whichever location is chosen.

## Command-line interface

The module has no import-time side effects, and its behavior does not depend on
the tool's directory depth.

```text
python3 -I <tooldir>/prepare_repo.py assemble --inputs PLAN.json --out REPO_DIR --record RECORD.json
python3 -I <tooldir>/prepare_repo.py verify   --inputs PLAN.json --out REPO_DIR --record RECORD.json
```

Rules:

- Exit 0 means success; any rejection exits non-zero with a specific message.
- There are no asserts, no network calls, no subprocesses and no fallback.
- The library API is `assemble(plan_path, out, record)` and
  `verify_repository(plan_path, out, record)`. Both raise `RepositoryError`
  with a reason code, for example `missing-input`, `integrity`,
  `provenance-conflict`, `byte-conflict`, `unsafe-path`, `unexpected-file`,
  `record-mismatch` or `stale-record`.

## Plan schema (`--inputs`, format 1)

The trusted activation controller writes the plan after checking the frozen
approval records. A plan is not dependency approval.

```json
{
  "format": 1,
  "roots": {"approved-index": "/abs/read-only/dir", "...": "..."},
  "entries": [
    {
      "path": "org/jetbrains/kotlin/kotlin-stdlib/2.1.20/kotlin-stdlib-2.1.20.jar",
      "kind": "binary",
      "source": {"root": "verified-runtime", "path": "relative/file"},
      "size": 1234,
      "sha256": "…",
      "sha512": "… or null",
      "provenance": {"url": "https://…", "identity": "approval record reference, e.g. ADR-0035 index entry / 8a87389 response / bundle 364a4201 file"}
    }
  ]
}
```

`kind` is `binary` or `metadata`.

## Validation

Validation is explicit and runs in this order:

1. **Schema.** Every output path must be a relative POSIX path, with no `..`,
   no absolute path, no backslash and no empty segment. Source paths must also
   be relative, and their resolved real path must stay inside the named root.
2. **Collection.** Collection is separate from validation:
   `by_path[path] = entry`. A path seen twice raises `provenance-conflict` if
   the provenance identity or URL differs (R03). It raises `byte-conflict` if
   the declared hashes or sizes differ for the same path. Identical duplicates
   are rejected too, for strictness.
3. **Inputs.** A missing source raises `missing-input`, locally (R02b). A size,
   SHA-256 or SHA-512 difference raises `integrity` (R02).
4. **Output.**
   - Copy into `REPO_DIR.partial-<random>`. Neither that path nor `REPO_DIR` may
     already exist.
   - Recheck that the written set equals the plan's path set exactly, with
     every hash matching.
   - Publish with an atomic `rename` to `REPO_DIR`.
   - Only then, write `RECORD.json` by atomic replace.

## Success record

The record is byte-identical between plain and `-O` runs:

```json
{"format": 1, "plan_sha256": "<sha256 of the plan file bytes>",
 "inventory": {"path": "sha256"}, "inventory_sha256": "<sha256 of canonical sorted inventory JSON>",
 "counts": {"binary": 0, "metadata": 0}}
```

It contains no timestamp, absolute path or interpreter mode. The run mode is
logged only in the caller's separate run log.

## Verification seam (R04)

`verify_repository` recomputes the plan SHA256 and requires all of the
following:

- the record's `plan_sha256` equals it;
- the record's inventory equals the plan's path→sha256 map;
- the actual files under `REPO_DIR` equal that set exactly, so extra, missing
  or changed files all reject.

A missing record, a record for another plan, or a tampered record rejects.
Directory existence and counts are never enough.

The RN Android build controller (currently `target/phase-seven-rn/android/build.py`)
calls it before its first Gradle subprocess, loading the tool by file path
with `importlib`.

## Controller and plan production

A task-specific controller (`target/phase-seven-rn/android/make-plan.py`)
builds the plan from:

- the ADR-0035 index plus `verified.json`;
- the 8a87389 records;
- compiler bundle `364a4201` / index `2182049f`;
- root's provenance-checked metadata lists.

It checks those frozen hashes explicitly. Acquiring missing bytes stays a
separate, explicit preparation step, never part of the assembler. The real
633/241 assembly is separate integration evidence.

## Tests (`<tooldir>/test_prepare_repo.py`, stdlib unittest)

The tests are offline and use no JAR, Gradle, ignored cache or shared input.

- Fixtures are synthetic: a tiny fake binary and metadata bytes with declared
  hashes, in fresh `tempfile` roots that are always cleaned.
- Each test runs the actual tool as a subprocess with `sys.executable -I`, and
  again with `-I -O`.

Cases:

- **R01:** identical inventories, bytes and byte-identical success records in
  both modes. The meaningful RED is against the new tool's compiling stub (its
  boundary exists, its behavior is not yet implemented), observed before
  implementing.
- **R02:** a corrupted binary, and separately a corrupted metadata input, raise
  `integrity` with no record and no published `REPO_DIR`.
- **R02b:** a missing binary, and separately a missing metadata input, raise
  `missing-input` with no record. There is no network or subprocess in the
  tool, so the check is static.
- **R03:** the same path with a different provenance identity raises
  `provenance-conflict`. Separately, the same path with different bytes or
  hashes raises `byte-conflict`.
- **R04:** the verify seam is exercised with:
  - a missing record;
  - a stale record (from another plan);
  - a tampered inventory;
  - an extra file;
  - a deleted file;
  - a mutated byte;
  - a partial directory.

  Each rejects. A controller-level check confirms that the build runner refuses
  before any Gradle subprocess, using a stubbed subprocess launcher.
- **Path safety:** absolute paths, escaping paths, and a source symlink
  pointing outside its root are each rejected.
- **v1 mechanism replay (separate test class):**
  - Uses a frozen copy at `<tooldir>/testdata/prepare-repo-v1.py.txt`, its
    SHA256 `f76c2a5c…` checked explicitly.
  - Stages a synthetic tree that mimics v1's hard-coded relative layout, then
    runs it plain and with `-O`.
  - Records its own counts, for example missing metadata and an accepted
    corruption under `-O`.
  - It is labelled a same-mechanism replay, **not** the exact 34/633 incident.

Commands to record with the regression:

```text
python3 -I <tooldir>/test_prepare_repo.py
python3 -I -O <tooldir>/test_prepare_repo.py
```

## Rollback

Remove the unpublished tool, its tests and derived outputs. The frozen inputs,
`repo-INVALID-O-run` and all logs stay.
