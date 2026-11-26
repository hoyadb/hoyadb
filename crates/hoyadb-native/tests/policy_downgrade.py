#!/usr/bin/env python3
"""TC-0044 CP14: real protected-store old/new artifact probe, task-owned files only.

Run with --old OLD_DYLIB --new NEW_DYLIB --directory NEW_EMPTY_PATH. Each artifact
runs in a fresh process; an old-build positive control is required before its
Discarded refusal counts. Source/lock/toolchain provenance is recorded by the
build caller; this probe records the exact binaries and observed file hashes.
"""
import argparse
import ctypes
import hashlib
import json
import pathlib
import platform
import subprocess
import sys


class Native:
    def __init__(self, path):
        self.lib = ctypes.CDLL(str(path))
        self.collector = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t)
        self.lib.hoya_v1_open.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p,
                                        ctypes.c_size_t, self.collector, ctypes.c_void_p]
        self.lib.hoya_v1_call.argtypes = [ctypes.c_void_p, ctypes.c_size_t,
                                        self.collector, ctypes.c_void_p]
        self.lib.hoya_v1_open.restype = ctypes.c_int32
        self.lib.hoya_v1_call.restype = ctypes.c_int32
        assert self.lib.hoya_v1_abi_version() == 1

    def invoke(self, op, owner, args, key=None):
        encoded = json.dumps({"abi": "1", "op": op, "owner": owner, "args": args},
                             separators=(",", ":"), sort_keys=True).encode()
        frame = ctypes.create_string_buffer(encoded)
        replies = []

        @self.collector
        def collect(_context, data, size):
            replies.append(json.loads(ctypes.string_at(data, size)))

        if key is None:
            status = self.lib.hoya_v1_call(frame, len(encoded), collect, None)
        else:
            secret = ctypes.create_string_buffer(key)
            status = self.lib.hoya_v1_open(frame, len(encoded), secret, len(key), collect, None)
        assert status == 0 and len(replies) == 1, (status, replies)
        return replies[0]

    def call(self, op, owner, args):
        reply = self.invoke(op, owner, args)
        assert reply.get("ok") is True, reply
        return reply["value"]


def open_args(directory):
    return {
        "engine": {"kind": "native", "directory": str(directory),
                   "name": "sealed.sqlite", "backup_excluded": False},
        "schema": {"version": "1", "tables": {"notes": {
            "columns": {"name": {"kind": "String", "optional": False}}, "relations": {}}}},
        "migrations": [], "namespace": "74747474747474747474747474747474",
        "options": {"capacity": "1000", "history": "100", "initial_delay_ms": "1000",
                    "multiplier": "2", "max_delay_ms": "60000"},
    }


def worker(artifact, directory, mode):
    native = Native(artifact)
    opened = native.invoke("open", None, open_args(directory), bytes(range(32)))
    if not opened.get("ok"):
        return {"open": opened}
    owner = opened["value"]
    if mode.startswith("create"):
        request = native.call("request", owner, {})
        assert native.call("begin", owner, {
            "request": request,
            "definition": {"name": "create-note-v1", "arguments": {}, "reference_paths": []},
            "value": {"kind": "object", "value": {}},
            "context": {"identity": {"kind": "null"}, "now_ms": "100"},
        }) is True
        row = native.call("scope_insert", owner, {"request": request, "table": "notes",
                          "fields": {"name": {"kind": "string", "value": "kept"}},
                          "proposed_id": None})
        result = native.call("finish", owner, {"request": request, "result": {"kind": "null"}})
        if mode == "create-discarded":
            report = native.call("discard", owner, {"key": result["key"]})
            assert report == {"kind": "kept_current", "discarded_keys": [result["key"]],
                              "unmapped": [row["id"]], "skipped_writes": "0"}, report
        native.call("set_conflict_policy", owner, {"policy": {"default": "stop", "commands": {}}})
    status = native.call("queue_status", owner, {})
    # Fetch through the public schema query, without accessing SQLite internals or keys.
    query = {"table": "notes", "predicates": [], "order": None, "limit": None, "source": None}
    rows = native.call("fetch", owner, {"query": query})
    native.call("close", owner, {})
    return {"open": opened, "status": status, "rows": rows}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory(directory):
    return {name: digest(directory / name) if (directory / name).exists() else None
            for name in ["sealed.sqlite", "sealed.sqlite-wal", "sealed.sqlite-shm"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", type=pathlib.Path)
    parser.add_argument("--new", type=pathlib.Path)
    parser.add_argument("--directory", type=pathlib.Path, required=True)
    parser.add_argument("--worker", choices=["create-control", "create-discarded", "open"])
    parser.add_argument("--artifact", type=pathlib.Path)
    args = parser.parse_args()
    if args.worker:
        print(json.dumps(worker(args.artifact, args.directory, args.worker), sort_keys=True))
        return
    if args.old is None or args.new is None:
        parser.error("--old and --new are required")
    directory = args.directory.resolve()
    directory.mkdir(mode=0o700, exist_ok=False)
    control = directory / "control"
    discarded = directory / "discarded"
    control.mkdir(mode=0o700)
    discarded.mkdir(mode=0o700)

    def run(artifact, path, mode):
        output = subprocess.check_output([sys.executable, __file__, "--worker", mode,
                                          "--artifact", str(artifact.resolve()),
                                          "--directory", str(path)], text=True)
        return json.loads(output)

    report = {"host": platform.platform(), "machine": platform.machine(),
              "artifacts": {"old": {"path": str(args.old), "sha256": digest(args.old)},
                            "new": {"path": str(args.new), "sha256": digest(args.new)}}}
    report["new_control"] = run(args.new, control, "create-control")
    report["new_discarded"] = run(args.new, discarded, "create-discarded")
    report["before_old"] = inventory(discarded)
    report["old_control"] = run(args.old, control, "open")
    assert report["old_control"]["open"].get("ok") is True, "invalid refusal probe: old positive control failed"
    report["old_discarded"] = run(args.old, discarded, "open")
    report["after_old"] = inventory(discarded)
    assert report["old_discarded"]["open"].get("ok") is False, report["old_discarded"]
    for name in ["sealed.sqlite", "sealed.sqlite-wal"]:
        assert report["before_old"][name] == report["after_old"][name], (name, report)
    report["new_reopen"] = run(args.new, discarded, "open")
    assert report["new_reopen"]["open"].get("ok") is True
    assert report["new_reopen"]["status"]["commands"][0]["state"] == "discarded"
    assert report["new_reopen"]["status"]["commands"][0]["rolled_back"] is False
    assert report["new_reopen"]["rows"] == report["new_discarded"]["rows"]
    (directory / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
