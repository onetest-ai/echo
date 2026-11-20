#!/usr/bin/env python3
"""A stand-in OpenTelemetry collector for the traces cases (Settings → Activity → Send traces elsewhere).

`python3 tests/fixtures/fake_otlp.py <port> <out.jsonl> [--code N]` serves http://127.0.0.1:<port> and writes one JSON
line a request to <out.jsonl>: {"path", "headers": {name: value}, "body": <the JSON sent>}; it answers N (200 by
default). Stop it with `kill`. Made for tests only: it keeps what it gets, in the run's own folder.
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port, out = int(sys.argv[1]), sys.argv[2]
code = int(sys.argv[sys.argv.index("--code") + 1]) if "--code" in sys.argv else 200


class Collector(BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length", 0) or 0))
        try:
            sent = json.loads(body or b"null")
        except ValueError:
            sent = body.decode("utf-8", "replace")
        with open(out, "a", encoding="utf-8") as f:
            f.write(json.dumps({"path": self.path, "headers": dict(self.headers), "body": sent}) + "\n")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"{}")

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", port), Collector).serve_forever()
