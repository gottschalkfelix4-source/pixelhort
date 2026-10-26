#!/usr/bin/env python3
"""Minimal OpenID Connect provider for testing the OAuth login flow.

python tests/oauth_mock.py --port 8999 --public http://host.docker.internal:8999
Then: python tests/oauth_mock.py --test --server http://localhost:2342 --admin-token <token>
"""
import argparse
import base64
import hashlib
import json
import sys
import threading
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer

CODES = {}


def make_handler(public, auth_public=None):
    class H(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def send_json(self, obj, status=200):
            body = json.dumps(obj).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            u = urllib.parse.urlparse(self.path)
            q = dict(urllib.parse.parse_qsl(u.query))
            if u.path == "/.well-known/openid-configuration":
                self.send_json({
                    "issuer": public,
                    "authorization_endpoint": f"{auth_public or public}/authorize",
                    "token_endpoint": f"{public}/token",
                    "userinfo_endpoint": f"{public}/userinfo",
                })
            elif u.path == "/authorize":
                code = "code-" + q["state"][:8]
                CODES[code] = q
                loc = q["redirect_uri"] + "?" + urllib.parse.urlencode({"code": code, "state": q["state"]})
                self.send_response(302)
                self.send_header("Location", loc)
                self.end_headers()
            elif u.path == "/userinfo":
                ok = self.headers.get("Authorization") == "Bearer at-123"
                self.send_json({"sub": "oidc-user-1", "email": "carol@example.com", "name": "Carol"} if ok else {}, 200 if ok else 401)
            else:
                self.send_json({}, 404)

        def do_POST(self):
            form = dict(urllib.parse.parse_qsl(self.rfile.read(int(self.headers["Content-Length"])).decode()))
            auth = CODES.pop(form.get("code"), None)
            challenge = base64.urlsafe_b64encode(hashlib.sha256(form.get("code_verifier", "").encode()).digest()).rstrip(b"=").decode()
            if not auth or auth.get("code_challenge") != challenge or form.get("client_secret") != "s3cret":
                self.send_json({"error": "invalid_grant"}, 400)
                return
            self.send_json({"access_token": "at-123", "token_type": "Bearer"})

    return H


def run_test(server, token, provider_local):
    def call(method, path, body=None):
        r = urllib.request.Request(server + path, method=method, data=json.dumps(body).encode() if body is not None else None,
                                   headers={"Content-Type": "application/json", "Authorization": "Bearer " + token})
        with urllib.request.urlopen(r) as res:
            return json.loads(res.read() or b"null")

    call("PUT", "/api/admin/config", {"oauth": {"enabled": True, "issuerUrl": "http://host.docker.internal:8999",
                                                "clientId": "pixelhort", "clientSecret": "s3cret"}})
    status = json.loads(urllib.request.urlopen(server + "/api/auth/status").read())
    assert status["oauthEnabled"], status
    redirect = "http://localhost:2342/auth/oauth-callback"
    url = json.loads(urllib.request.urlopen(server + "/api/auth/oauth/authorize?redirectUri=" + urllib.parse.quote(redirect)).read())["url"]
    assert "code_challenge=" in url and "state=" in url, url
    # Follow the provider redirect manually (browser step).
    local = url.replace("http://host.docker.internal:8999", provider_local)

    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a, **k):
            return None

    try:
        urllib.request.build_opener(NoRedirect).open(local)
        raise SystemExit("expected redirect")
    except urllib.error.HTTPError as e:
        callback = e.headers["Location"]
    r = urllib.request.Request(server + "/api/auth/oauth/callback", data=json.dumps({"url": callback}).encode(),
                               headers={"Content-Type": "application/json"}, method="POST")
    login = json.loads(urllib.request.urlopen(r).read())
    assert login["user"]["email"] == "carol@example.com" and login["user"]["oauthId"] == "oidc-user-1", login
    me = urllib.request.Request(server + "/api/users/me", headers={"Authorization": "Bearer " + login["accessToken"]})
    assert json.loads(urllib.request.urlopen(me).read())["name"] == "Carol"
    # Replaying the same state must fail.
    try:
        urllib.request.urlopen(urllib.request.Request(server + "/api/auth/oauth/callback", data=json.dumps({"url": callback}).encode(),
                                                      headers={"Content-Type": "application/json"}, method="POST"))
        raise SystemExit("state replay accepted")
    except urllib.error.HTTPError as e:
        assert e.code == 401, e.code
    call("PUT", "/api/admin/config", {"oauth": {"enabled": False}})
    print("OAuth flow OK: discovery, PKCE, token exchange, userinfo, auto-register, replay protection")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, default=8999)
    p.add_argument("--public", default="http://host.docker.internal:8999")
    p.add_argument("--auth-public", help="browser-facing base URL for /authorize (e.g. http://10.0.2.2:8999 for the Android emulator)")
    p.add_argument("--test", action="store_true")
    p.add_argument("--server", default="http://localhost:2342")
    p.add_argument("--admin-token")
    a = p.parse_args()
    srv = HTTPServer(("0.0.0.0", a.port), make_handler(a.public, a.auth_public))
    if a.test:
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        run_test(a.server, a.admin_token, f"http://localhost:{a.port}")
        sys.exit(0)
    srv.serve_forever()
