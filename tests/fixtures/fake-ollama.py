#!/usr/bin/env python3
"""A stand-in Ollama server for the context-window checks (#224), on 127.0.0.1 only.

    python3 tests/fixtures/fake-ollama.py <port> --ctx N [--model NAME] [--loaded] [--log FILE]
    python3 tests/fixtures/fake-ollama.py <port> --not-ollama [--log FILE]

`/api/ps` lists NAME with `context_length` N once it is loaded: by `POST /api/generate` for NAME,
or from the start with `--loaded`. `--not-ollama`: every path answers 404, as a server with only
Anthropic's Messages API would. Each request is logged as `METHOD PATH` (a POST also its body) (to FILE, else stderr).
"""
import argparse
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

ap = argparse.ArgumentParser()
ap.add_argument("port", type=int)
ap.add_argument("--ctx", type=int, default=262144)
ap.add_argument("--model", default="small-model")
ap.add_argument("--loaded", action="store_true")
ap.add_argument("--not-ollama", action="store_true")
ap.add_argument("--log")
a = ap.parse_args()
loaded = a.loaded


def log(line):
    if a.log:
        with open(a.log, "a") as f:
            f.write(line + "\n")
    else:
        print(line, file=sys.stderr, flush=True)


class H(BaseHTTPRequestHandler):
    def reply(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        log(f"GET {self.path}")
        if a.not_ollama or self.path != "/api/ps":
            return self.reply(404, {"error": "not found"})
        models = [{"name": a.model, "model": a.model, "context_length": a.ctx}] if loaded else []
        self.reply(200, {"models": models})

    def do_POST(self):
        global loaded
        body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        log(f"POST {self.path} {body.decode(errors='replace')}")
        if a.not_ollama or self.path != "/api/generate":
            return self.reply(404, {"error": "not found"})
        if json.loads(body or b"{}").get("model") != a.model:
            return self.reply(404, {"error": "model not found"})
        loaded = True
        self.reply(200, {"model": a.model, "response": "", "done": True})

    def log_message(self, *_):
        pass


HTTPServer(("127.0.0.1", a.port), H).serve_forever()
