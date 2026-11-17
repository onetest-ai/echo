#!/usr/bin/env python3
"""A made-up voice server for plugin cases: `tests/fixtures/ws_echo.py <port> <log>`, on 127.0.0.1 only.

- WebSocket on any path: each text message is JSON-RPC 2.0; it answers `{"jsonrpc": "2.0", "id": <id>, "result":
  {"method": <method>, "params": <params>}}` (an error object for a message that isn't JSON).
- `GET /ping`: `{"ok": true}` with `Access-Control-Allow-Origin: *` (a page's `fetch` passes CORS).

Every connection is a line in <log> (`ws <path>`, `get <path>`, `rpc <method>`), so a case can tell a connection the
page's policy blocked (no line) from one that failed. Python's standard library only; one thread per connection.
"""
import base64
import hashlib
import json
import socketserver
import struct
import sys

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


class Handler(socketserver.StreamRequestHandler):
    def note(self, line):
        with open(self.server.log, "a") as f:
            f.write(line + "\n")

    def handle(self):
        head = self.rfile.readline().decode("latin-1").split()
        if len(head) < 2:
            return
        path = head[1]
        headers = {}
        while True:
            line = self.rfile.readline().decode("latin-1").strip()
            if not line:
                break
            k, _, v = line.partition(":")
            headers[k.strip().lower()] = v.strip()
        key = headers.get("sec-websocket-key")
        if not key:
            self.note(f"get {path}")
            body = b'{"ok": true}' if path == "/ping" else b'{"error": "not found"}'
            status = "200 OK" if path == "/ping" else "404 Not Found"
            self.wfile.write(f"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {len(body)}\r\nConnection: close\r\n\r\n".encode() + body)
            return
        accept = base64.b64encode(hashlib.sha1((key + GUID).encode()).digest()).decode()
        self.wfile.write(f"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n".encode())
        self.note(f"ws {path}")
        while True:
            frame = self.read_frame()
            if frame is None:
                return
            opcode, data = frame
            if opcode == 8:  # close
                self.send(8, data[:2])
                return
            if opcode == 9:  # ping
                self.send(10, data)
                continue
            if opcode != 1:
                continue
            try:
                msg = json.loads(data.decode())
                self.note(f"rpc {msg.get('method')}")
                answer = {"jsonrpc": "2.0", "id": msg.get("id"), "result": {"method": msg.get("method"), "params": msg.get("params")}}
            except (ValueError, AttributeError):
                answer = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}
            self.send(1, json.dumps(answer).encode())

    def read_frame(self):
        b = self.rfile.read(2)
        if len(b) < 2:
            return None
        opcode, n = b[0] & 0x0F, b[1] & 0x7F
        if n == 126:
            n = struct.unpack(">H", self.rfile.read(2))[0]
        elif n == 127:
            n = struct.unpack(">Q", self.rfile.read(8))[0]
        mask = self.rfile.read(4) if b[1] & 0x80 else b"\0\0\0\0"
        data = bytes(c ^ mask[i % 4] for i, c in enumerate(self.rfile.read(n)))
        return opcode, data

    def send(self, opcode, data):
        n = len(data)
        head = bytes([0x80 | opcode]) + (bytes([n]) if n < 126 else bytes([126]) + struct.pack(">H", n))
        self.wfile.write(head + data)


class Server(socketserver.ThreadingTCPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    port, log = int(sys.argv[1]), sys.argv[2]
    with Server(("127.0.0.1", port), Handler) as s:
        s.log = log
        s.serve_forever()
