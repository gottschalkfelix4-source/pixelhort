#!/usr/bin/env python3
"""Casts a photo and a video to a real DLNA renderer and checks that it played them.

Requires a running Pixelhort stack with at least one image and one video, and the
renderer from renderer.Dockerfile running as container `tv-renderer` in the same
Docker network as the server.

python tests/dlna/cast_test.py --email admin@example.com --password …
"""
import argparse
import json
import subprocess
import sys
import time
import urllib.request

p = argparse.ArgumentParser()
p.add_argument("--server", default="http://localhost:2342")
p.add_argument("--email", required=True)
p.add_argument("--password", required=True)
p.add_argument("--renderer", default="tv-renderer")
a = p.parse_args()


def call(method, path, body=None, token=None):
    req = urllib.request.Request(a.server + path, method=method,
                                 data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + token} if token else {})})
    with urllib.request.urlopen(req, timeout=60) as res:
        return json.loads(res.read() or b"null")


def renderer_log(since):
    out = subprocess.run(["docker", "logs", "--since", since, a.renderer], capture_output=True, text=True)
    return out.stdout + out.stderr


token = call("POST", "/api/auth/login", {"email": a.email, "password": a.password})["accessToken"]
devices = call("GET", "/api/cast/devices?refresh=true", token=token)
assert devices, "no DLNA renderer discovered"
device = devices[0]
print("discovered:", device["name"], device["model"])

ok = True
for kind in ["IMAGE", "VIDEO"]:
    asset = call("POST", "/api/search/metadata", {"type": kind, "size": 1}, token)["assets"][0]
    start = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(time.time() - 1))
    res = call("POST", f"/api/cast/devices/{device['id']}/play", {"assetId": asset["id"]}, token)
    time.sleep(3)
    log = renderer_log(start)
    played = asset["id"] in log and "TransportState: PLAYING" in log and "End-of-stream" in log
    print(f"{kind:5} {asset['originalFileName']:20} -> {res['mimeType']:10} played={played}")
    ok &= played

state = call("GET", f"/api/cast/devices/{device['id']}/status", token=token)["state"]
print("final state:", state)
sys.exit(0 if ok else 1)
