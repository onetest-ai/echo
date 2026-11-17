#!/usr/bin/env python3
"""A stand-in Microsoft Graph for plugin cases, on 127.0.0.1 only: made-up acme.example mail and one Teams chat.

    python3 tests/fixtures/fake_graph.py <port> [--log FILE]

A debug build with `ECHOCTL_TEST_GRAPH=http://127.0.0.1:<port>` is signed in to it (no real sign-in, no tokens) and
sends every Graph request here, the path after `/v1.0`. It serves what `ctx.m365.search`, `ctx.m365.mailThread` and
`ctx.m365.chat` ask for:

- `POST /search/query` — mail whose subject or text has every word of the query (`kind` mail), the chat's messages
  likewise (chats); dates and KQL words (`received>=…`, `sent>=…`) are ignored
- `GET /me/messages/<id>` — one mail; `GET /me/messages?$filter=conversationId eq '<c>'` — its conversation
- `GET /me/messages/<id>/attachments` — its attachments (an inline picture among them)
- `GET /chats/<id>` (members) and `GET /chats/<id>/messages` — the chat, its messages dated today
- `GET /me` — the user

Anything else answers 404 with Graph's error shape. Each request is logged as `METHOD PATH` (to FILE, else stderr).
Everything here is invented (tests/app_profile.md, rule 1).
"""
import argparse
import datetime
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, unquote, urlsplit

ap = argparse.ArgumentParser()
ap.add_argument("port", type=int)
ap.add_argument("--log")
a = ap.parse_args()


def who(name, address):
    return {"emailAddress": {"name": name, "address": address}}


SAM = who("Sam Rivera", "sam@initech.example")
JANE = who("Jane Doe", "jane@acme.example")
CONV = "conv-acme-pilot"
MAIL = [
    {"id": "AAMk-acme-pilot-1", "conversationId": CONV, "internetMessageId": "<pilot-1@acme.example>",
     "subject": "Acme pilot plan", "from": JANE, "toRecipients": [SAM], "ccRecipients": [],
     "receivedDateTime": "2026-11-10T09:00:00Z", "body": {"contentType": "text", "content": "Here is the pilot plan for the Acme rollout."},
     "webLink": "https://outlook.example/acme-pilot-1", "hasAttachments": True},
    {"id": "AAMk-acme-pilot-2", "conversationId": CONV, "internetMessageId": "<pilot-2@initech.example>",
     "subject": "RE: Acme pilot plan", "from": SAM, "toRecipients": [JANE], "ccRecipients": [],
     "receivedDateTime": "2026-11-11T10:30:00Z", "body": {"contentType": "text", "content": "Thanks, the pilot starts on Monday.\nFrom: Jane Doe\nHere is the pilot plan"},
     "webLink": "https://outlook.example/acme-pilot-2", "hasAttachments": False},
]
ATTACHMENTS = {"AAMk-acme-pilot-1": [{"name": "acme-pilot-plan.pdf", "isInline": False}, {"name": "signature.png", "isInline": True}]}
CHAT = "19:acme-pilot@thread.v2"


def chat_messages():
    today = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0)
    at = lambda h: (today - datetime.timedelta(hours=h)).isoformat().replace("+00:00", "Z")
    return [
        {"id": "1700000000001", "messageType": "message", "createdDateTime": at(2), "from": {"user": {"displayName": "Jane Doe"}},
         "body": {"contentType": "html", "content": "<p>Is the pilot plan final?</p>"}, "attachments": []},
        {"id": "1700000000002", "messageType": "message", "createdDateTime": at(1), "from": {"user": {"displayName": "Sam Rivera"}},
         "body": {"contentType": "text", "content": "Yes, the pilot starts on Monday."}, "attachments": [{"name": "acme-pilot-plan.pdf"}]},
    ]


def log(line):
    if a.log:
        with open(a.log, "a") as f:
            f.write(line + "\n")
    else:
        print(line, file=sys.stderr, flush=True)


def has_words(query, *texts):
    words = [w for w in query.lower().split() if ">=" not in w and ":" not in w]
    hay = " ".join(texts).lower()
    return all(w in hay for w in words)


class H(BaseHTTPRequestHandler):
    def reply(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def missing(self):
        self.reply(404, {"error": {"code": "ErrorItemNotFound", "message": "Not in the fake Microsoft 365"}})

    def log_message(self, *_):
        pass

    def do_GET(self):
        u = urlsplit(self.path)
        path, q = unquote(u.path), parse_qs(u.query)
        log(f"GET {self.path}")
        parts = path.strip("/").split("/")
        if parts == ["me"]:
            return self.reply(200, {"displayName": "Sam Rivera", "mail": "sam@initech.example", "userPrincipalName": "sam@initech.example"})
        if parts == ["me", "messages"]:
            f = q.get("$filter", [""])[0]
            conv = f.split("'")[1] if f.startswith("conversationId eq '") else None
            return self.reply(200, {"value": [m for m in MAIL if m["conversationId"] == conv]})
        if len(parts) >= 3 and parts[:2] == ["me", "messages"]:
            m = next((m for m in MAIL if m["id"] == parts[2]), None)
            if m is None:
                return self.missing()
            if parts[3:] == ["attachments"]:
                return self.reply(200, {"value": ATTACHMENTS.get(m["id"], [])})
            return self.reply(200, m) if len(parts) == 3 else self.missing()
        if len(parts) >= 2 and parts[0] == "chats" and parts[1] == CHAT:
            if len(parts) == 2:
                return self.reply(200, {"id": CHAT, "topic": "Acme pilot", "webUrl": "https://teams.example/acme-pilot",
                                        "members": [{"displayName": "Jane Doe", "email": "jane@acme.example"}, {"displayName": "Sam Rivera", "email": "sam@initech.example"}]})
            if parts[2:] == ["messages"]:
                return self.reply(200, {"value": sorted(chat_messages(), key=lambda m: m["createdDateTime"], reverse=True)})
        self.missing()

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        log(f"POST {self.path} {body.decode(errors='replace')}")
        if urlsplit(self.path).path != "/search/query":
            return self.missing()
        req = json.loads(body or b"{}")["requests"][0]
        query, entity = req["query"]["queryString"], req["entityTypes"][0]
        if entity == "message":
            hits = [{"hitId": m["id"], "summary": m["body"]["content"][:80], "resource": {k: m[k] for k in ("subject", "from", "receivedDateTime", "conversationId", "webLink")}}
                    for m in MAIL if has_words(query, m["subject"], m["body"]["content"])]
        elif entity == "chatMessage":
            hits = [{"hitId": m["id"], "summary": m["body"]["content"][:80], "resource": {"id": m["id"], "chatId": CHAT, "from": m["from"], "createdDateTime": m["createdDateTime"]}}
                    for m in chat_messages() if has_words(query, m["body"]["content"])]
        else:
            hits = []
        self.reply(200, {"value": [{"hitsContainers": [{"hits": hits, "total": len(hits)}]}]})


HTTPServer(("127.0.0.1", a.port), H).serve_forever()
