#!/usr/bin/env python3
"""A stand-in for GitHub's device sign-in (tests/cases/plugins/TC-025), on 127.0.0.1 only.

    python3 tests/fixtures/fake-github-device.py <port> <dir>

`POST /login/device/code` answers the code `WDJB-MJHT` (device code `dev-madeup-1`, interval 1 s). `POST
/login/oauth/access_token` answers `authorization_pending` until the user "confirms on GitHub": the file
`<dir>/approve` makes it answer the made-up token `gho_madeupdevice0000000000000000000000000` (41 characters, scope
`repo,read:org`), `<dir>/deny` makes it answer `access_denied`. Each request is logged to `<dir>/requests.log` as
`PATH BODY`. Every token here is made up.
"""
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port, d = int(sys.argv[1]), sys.argv[2]
os.makedirs(d, exist_ok=True)


class H(BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length") or 0)).decode()
        with open(os.path.join(d, "requests.log"), "a") as f:
            f.write(f"{self.path} {body}\n")
        if self.path == "/login/device/code":
            a = {"device_code": "dev-madeup-1", "user_code": "WDJB-MJHT", "verification_uri": "https://github.com/login/device", "expires_in": 900, "interval": 1}
        elif self.path == "/login/oauth/access_token":
            if os.path.exists(os.path.join(d, "deny")):
                a = {"error": "access_denied"}
            elif os.path.exists(os.path.join(d, "approve")):
                a = {"access_token": "gho_madeupdevice0000000000000000000000000", "token_type": "bearer", "scope": "repo,read:org"}
            else:
                a = {"error": "authorization_pending"}
        else:
            self.send_response(404)
            self.end_headers()
            return
        b = json.dumps(a).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def log_message(self, *_):
        pass


HTTPServer(("127.0.0.1", port), H).serve_forever()
