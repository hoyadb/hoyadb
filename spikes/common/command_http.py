"""API-0008/API-0009: synthetic loopback sync transport, no payload logging.

Run: python3 spikes/common/command_http.py --port 8774
This is an experimental mixed-write/paged-pull fixture, not a backend/authentication service.
"""
import argparse
import functools
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

LIMIT = 1048576
MODES = {"applied", "mixed", "partial", "duplicate", "conflict", "unavailable"}


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate key")
        result[key] = value
    return result


def response(body, mode="applied"):
    try:
        if len(body) > LIMIT or mode not in MODES:
            raise ValueError("invalid request")
        def invalid_constant(_value):
            raise ValueError("nonfinite number")
        commands = json.loads(body.decode("utf-8"), object_pairs_hook=unique,
                              parse_constant=invalid_constant)
        if not isinstance(commands, list) or not commands:
            raise ValueError("nonempty array required")
        for command in commands:
            if not isinstance(command, dict):
                raise ValueError("envelope required")
            entity = "writeId" in command
            if entity:
                if (set(command) != {"table", "op", "recordId", "changes", "removedFields",
                                     "clientTimestamp", "schemaVersion", "context", "writeId"}
                        or command["op"] not in {"create", "update", "delete"}
                        or any(not isinstance(command[key], str) or not command[key]
                               for key in ["table", "recordId", "writeId", "clientTimestamp"])
                        or not isinstance(command["changes"], dict)
                        or not isinstance(command["removedFields"], list)
                        or type(command["schemaVersion"]) is not int
                        or not isinstance(command["context"], dict)):
                    raise ValueError("entity envelope required")
            elif (set(command) != {
                    "command", "idempotencyKey", "schemaVersion", "args", "context"}
                    or not isinstance(command["command"], str)
                    or not isinstance(command["idempotencyKey"], str)
                    or not command["idempotencyKey"]
                    or type(command["schemaVersion"]) is not int
                    or not isinstance(command["args"], dict)
                    or not isinstance(command["context"], dict)):
                raise ValueError("command envelope required")
        if mode == "unavailable":
            return 503, b'{"error":"fixture unavailable"}'
        results = []
        for index, command in enumerate(commands):
            state = ["applied", "corrected", "rejected"][index % 3] if mode == "mixed" else "applied"
            entity = "writeId" in command
            result = ({key: command[key] for key in ["table", "recordId", "writeId"]}
                      if entity else {"idempotencyKey": command["idempotencyKey"]})
            result["status"] = state
            if state == "rejected":
                result["reason"] = "synthetic business rejection"
            else:
                result["result"] = {"id": command["recordId"] if entity else "server:" + command["idempotencyKey"],
                                    "echo": command["changes"] if entity else command["args"]}
                if entity:
                    result["result"]["created"] = command["op"] == "create"
            results.append(result)
        results.reverse()
        if mode == "partial":
            results = results[:1]
        elif mode == "duplicate":
            results += list(results)
        elif mode == "conflict":
            correlation = {key: value for key, value in results[0].items()
                           if key not in {"status", "result", "reason"}}
            results.append({**correlation, "status": "rejected", "reason": "synthetic conflicting result"})
        return 200, json.dumps(results, ensure_ascii=False, allow_nan=False,
                               separators=(",", ":")).encode("utf-8")
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError):
        return 400, b'{"error":"invalid fixture request"}'


def pull_response(since=None, mode="normal"):
    """Synthetic typed pages: stable opaque continuation, no client-supplied data."""
    if mode not in {"normal", "large", "unavailable"}:
        return 400, b'{"error":"invalid fixture request"}'
    if mode == "unavailable":
        return 503, b'{"error":"fixture unavailable"}'
    if since not in {None, "", "fixture:page-one", "fixture:complete"}:
        return 400, b'{"error":"invalid fixture request"}'
    if since == "fixture:complete":
        page = {"checkpoint": since, "complete": True, "changes": []}
    else:
        first = since in {None, ""}
        value = "first 東京\u0000🌿" if first else "second 東京\u0000🌿"
        if mode == "large":
            value = "🌿\n" * 60000
        time = "1000" if first else "1001"
        page = {"checkpoint": "fixture:page-one" if first else "fixture:complete",
                "complete": not first,
                "changes": [{"table": "customers", "record_id": "server:pull-customer",
                             "op": "create" if first else "update",
                             "changes": {"country": {"kind": "string", "value": value}},
                             "removed_fields": [], "timestamp_ms": time,
                             "field_timestamps": {"country": time}}]}
    raw = json.dumps(page, ensure_ascii=False, allow_nan=False,
                     separators=(",", ":")).encode("utf-8")
    if len(raw) > LIMIT:
        return 400, b'{"error":"invalid fixture request"}'
    return 200, raw


class Handler(SimpleHTTPRequestHandler):
    def log_message(self, _format, *_args):
        pass

    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Hoya-Fixture")
        self.send_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(204)
        self.end_headers()

    def do_POST(self):
        if self.path != "/hoyadb/commands":
            self.send_error(404)
            return
        try:
            size = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            size = -1
        if size < 0 or size > LIMIT:
            code, body = 400, b'{"error":"invalid fixture request"}'
        else:
            code, body = response(self.rfile.read(size), self.headers.get("X-Hoya-Fixture", "applied"))
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        request = urlsplit(self.path)
        if request.path != "/hoyadb/changes":
            return super().do_GET()
        query = parse_qs(request.query, keep_blank_values=True)
        if set(query) - {"since"} or len(query.get("since", [])) > 1:
            code, body = 400, b'{"error":"invalid fixture request"}'
        else:
            code, body = pull_response(query.get("since", [None])[0],
                                       self.headers.get("X-Hoya-Fixture", "normal"))
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8774)
    args = parser.parse_args()
    assets = Path(__file__).resolve().parents[1] / "storage-browser"
    server = ThreadingHTTPServer(("127.0.0.1", args.port), functools.partial(Handler, directory=str(assets)))
    print(f"Synthetic sync reference: http://127.0.0.1:{server.server_port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
