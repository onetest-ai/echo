#!/usr/bin/env python3
"""A virtual model for Echo's tests: a local server that speaks the model APIs Echo and Claude Code use, answers from
a script, and logs every request. Only the model is virtual: Claude Code, its tools, Echo's guard and the app run
for real (#275).

  tests/fixtures/model_server.py <dir> [--port N]     serve until killed; writes <dir>/model.url when listening

APIs:
  POST /v1/messages              Anthropic Messages, plain or streamed (SSE), text and tool_use answers
  POST /v1/messages/count_tokens a rough count
  GET  /api/ps, POST /api/generate, GET /api/tags   enough of Ollama for `echoctl doctor`'s context check

The script, <dir>/model-script.json (read on every request, so a case can change it between steps):
  {"model": {"context_length": 32768},
   "rules": [{"when": ["phrase", ...], "steps": [ANSWER, ANSWER, ...]}, ...]}
A request matches the first rule all of whose phrases are in its system prompt or messages; its step is the number
of assistant turns already in the request (0 for the first call, 1 after the first tool result, ...), and the last
step repeats. An ANSWER is {"text": "..."}, {"tool": "Read", "input": {...}} or {"blocks": [content blocks]}; a
request no rule matches gets the text "virtual model: no script for this request". A rule's "vars" ({"dir":
"<regex with one group>"}) are searched in the request, and "{dir}" in its answers becomes what they found.

The log, <dir>/model-log.jsonl: one line per request — when, path, model, stream, system, messages, tool names,
the rule and step that answered and the answer.
"""
import argparse
import json
import os
import re
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

NO_SCRIPT = {"text": "virtual model: no script for this request"}


def text_of(content):
    """Every text in a message's content (a string or blocks), tool inputs and tool results included."""
    if isinstance(content, str):
        return content
    out = []
    for b in content or []:
        if not isinstance(b, dict):
            continue
        for k in ("text", "thinking"):
            if isinstance(b.get(k), str):
                out.append(b[k])
        if "input" in b:
            out.append(json.dumps(b["input"], ensure_ascii=False))
        if "content" in b:
            out.append(text_of(b["content"]))
    return "\n".join(out)


def answer_for(req, script):
    """The rule, the step and the answer for a Messages request."""
    system = req.get("system")
    hay = text_of(system if isinstance(system, list) else [{"text": system or ""}])
    msgs = req.get("messages") or []
    hay += "\n" + "\n".join(text_of(m.get("content")) for m in msgs)
    step = sum(1 for m in msgs if m.get("role") == "assistant")
    for i, rule in enumerate(script.get("rules", [])):
        when = rule.get("when", [])
        if all(w in hay for w in ([when] if isinstance(when, str) else when)):
            steps = rule.get("steps") or [NO_SCRIPT]
            found = {}
            for name, rx in (rule.get("vars") or {}).items():
                m = re.search(rx, hay)
                found[name] = m.group(1) if m else ""
            return i, step, fill(steps[min(step, len(steps) - 1)], found)
    return None, step, NO_SCRIPT


def fill(v, found):
    """`{name}` in every string of an answer, from the rule's vars."""
    if isinstance(v, str):
        for k, x in found.items():
            v = v.replace("{" + k + "}", x)
        return v
    if isinstance(v, list):
        return [fill(x, found) for x in v]
    if isinstance(v, dict):
        return {k: fill(x, found) for k, x in v.items()}
    return v


def blocks_of(answer, n):
    if "blocks" in answer:
        return answer["blocks"]
    if "tool" in answer:
        pre = [{"type": "text", "text": answer["say"]}] if answer.get("say") else []
        return pre + [{"type": "tool_use", "id": f"toolu_virtual_{n:04d}", "name": answer["tool"], "input": answer.get("input", {})}]
    return [{"type": "text", "text": answer.get("text", "")}]


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, addr, root):
        super().__init__(addr, Handler)
        self.root = root
        self.lock = threading.Lock()
        self.n = 0

    def script(self):
        try:
            with open(os.path.join(self.root, "model-script.json"), encoding="utf-8") as f:
                return json.load(f)
        except (OSError, ValueError):
            return {}

    def log(self, entry):
        with self.lock:
            with open(os.path.join(self.root, "model-log.jsonl"), "a", encoding="utf-8") as f:
                f.write(json.dumps(entry, ensure_ascii=False) + "\n")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(n) if n else b""
        try:
            return json.loads(raw or b"{}")
        except ValueError:
            return {}

    def send_json(self, code, obj):
        data = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = self.path.split("?")[0]
        ctx = self.server.script().get("model", {}).get("context_length", 32768)
        if path == "/api/ps":
            return self.send_json(200, {"models": [{"name": "virtual", "model": "virtual", "context_length": ctx}]})
        if path == "/api/tags":
            return self.send_json(200, {"models": [{"name": "virtual", "model": "virtual"}]})
        if path in ("/", "/health"):
            return self.send_json(200, {"ok": True})
        self.server.log({"at": time.time(), "path": path, "method": "GET"})
        return self.send_json(404, {"error": "not here"})

    def do_HEAD(self):
        self.send_response(200)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_POST(self):
        path = self.path.split("?")[0]
        req = self.body()
        if path == "/api/generate":
            return self.send_json(200, {"model": req.get("model", "virtual"), "response": "", "done": True})
        if path == "/v1/messages/count_tokens":
            return self.send_json(200, {"input_tokens": max(1, len(json.dumps(req)) // 4)})
        if path != "/v1/messages":
            self.server.log({"at": time.time(), "path": path, "method": "POST"})
            return self.send_json(404, {"error": "not here"})
        with self.server.lock:
            self.server.n += 1
            n = self.server.n
        rule, step, answer = answer_for(req, self.server.script())
        blocks = blocks_of(answer, n)
        stop = "tool_use" if any(b.get("type") == "tool_use" for b in blocks) else "end_turn"
        self.server.log({"at": time.time(), "n": n, "path": path, "model": req.get("model"), "stream": bool(req.get("stream")),
                         "system": req.get("system"), "messages": req.get("messages"),
                         "tools": [t.get("name") for t in req.get("tools") or []], "rule": rule, "step": step, "answer": blocks})
        msg = {"id": f"msg_virtual_{n:04d}", "type": "message", "role": "assistant", "model": req.get("model") or "virtual",
               "content": blocks, "stop_reason": stop, "stop_sequence": None,
               "usage": {"input_tokens": max(1, len(json.dumps(req)) // 4), "output_tokens": max(1, len(json.dumps(blocks)) // 4)}}
        if not req.get("stream"):
            return self.send_json(200, msg)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()

        def ev(kind, data):
            self.wfile.write(f"event: {kind}\ndata: {json.dumps(data)}\n\n".encode())

        ev("message_start", {"type": "message_start", "message": {**msg, "content": [], "stop_reason": None,
                                                                    "usage": {"input_tokens": msg["usage"]["input_tokens"], "output_tokens": 1}}})
        for i, b in enumerate(blocks):
            if b["type"] == "text":
                ev("content_block_start", {"type": "content_block_start", "index": i, "content_block": {"type": "text", "text": ""}})
                ev("content_block_delta", {"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": b["text"]}})
            elif b["type"] == "tool_use":
                ev("content_block_start", {"type": "content_block_start", "index": i,
                                           "content_block": {"type": "tool_use", "id": b["id"], "name": b["name"], "input": {}}})
                ev("content_block_delta", {"type": "content_block_delta", "index": i,
                                           "delta": {"type": "input_json_delta", "partial_json": json.dumps(b["input"])}})
            ev("content_block_stop", {"type": "content_block_stop", "index": i})
        ev("message_delta", {"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": None},
                             "usage": {"output_tokens": msg["usage"]["output_tokens"]}})
        ev("message_stop", {"type": "message_stop"})
        self.wfile.flush()
        self.close_connection = True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--port", type=int, default=0)
    a = ap.parse_args()
    os.makedirs(a.dir, exist_ok=True)
    srv = Server(("127.0.0.1", a.port), a.dir)
    url = f"http://127.0.0.1:{srv.server_address[1]}"
    with open(os.path.join(a.dir, "model.url"), "w") as f:
        f.write(url + "\n")
    print(url, flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
