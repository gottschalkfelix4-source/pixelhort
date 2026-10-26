#!/usr/bin/env python3
"""End-to-end test of a running Pixelhort stack (server + db + ml).

Usage: python tests/e2e.py --server http://localhost:2342 --media <dir> [--ext-path /mnt/ext]

<dir> must contain the files produced by tests/prepare_media.sh. The script
uses only the Python standard library. Exit code 0 = all checks passed.
"""
import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
import uuid

P = argparse.ArgumentParser()
P.add_argument("--server", default="http://localhost:2342")
P.add_argument("--metrics", default="http://localhost:9464/metrics")
P.add_argument("--media", required=True)
P.add_argument("--ext-path", default="/mnt/ext")
P.add_argument("--timeout", type=int, default=600)
A = P.parse_args()

RESULTS = []


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond)))
    print(("  PASS " if cond else "  FAIL ") + name + ("" if cond else f"  -> {detail}"))
    return bool(cond)


class Resp:
    def __init__(self, status, body, headers):
        self.status, self.body = status, body
        self.headers = {k.lower(): v for k, v in headers.items()}

    def json(self):
        return json.loads(self.body) if self.body else None


def req(method, path, body=None, token=None, headers=None, raw=None, ctype=None, base=None):
    url = path if path.startswith("http") else (base or A.server) + path
    h = dict(headers or {})
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        h["Content-Type"] = "application/json"
    if raw is not None:
        data = raw
        h["Content-Type"] = ctype
    if token:
        h["Authorization"] = "Bearer " + token
    r = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(r, timeout=120) as res:
            return Resp(res.status, res.read(), dict(res.headers))
    except urllib.error.HTTPError as e:
        return Resp(e.code, e.read(), dict(e.headers))


def upload(token, path, fields=None, query=""):
    boundary = uuid.uuid4().hex
    parts = []
    fields = dict(fields or {})
    fields.setdefault("deviceAssetId", os.path.basename(path))
    fields.setdefault("deviceId", "e2e")
    fields.setdefault("fileCreatedAt", "2024-01-01T00:00:00Z")
    fields.setdefault("fileModifiedAt", "2024-01-01T00:00:00Z")
    for k, v in fields.items():
        parts.append(f"--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n".encode())
    with open(path, "rb") as f:
        content = f.read()
    parts.append(
        f"--{boundary}\r\nContent-Disposition: form-data; name=\"assetData\"; filename=\"{os.path.basename(path)}\"\r\n"
        f"Content-Type: application/octet-stream\r\n\r\n".encode() + content + b"\r\n"
    )
    parts.append(f"--{boundary}--\r\n".encode())
    return req("POST", "/api/assets" + query, raw=b"".join(parts), ctype=f"multipart/form-data; boundary={boundary}", token=token)


def sha1(path):
    with open(path, "rb") as f:
        return hashlib.sha1(f.read()).hexdigest()


def media(name):
    return os.path.join(A.media, name)


def wait_jobs(token, label):
    deadline = time.time() + A.timeout
    while time.time() < deadline:
        jobs = req("GET", "/api/jobs", token=token).json()
        busy = sum(q["waiting"] + q["active"] for q in jobs.values())
        if busy == 0:
            failed = {k: v["failed"] for k, v in jobs.items() if v["failed"]}
            print(f"  jobs idle after {label}; failed: {failed or 'none'}")
            return jobs
        time.sleep(2)
    raise SystemExit("jobs did not finish in time")


def main():
    print("== auth")
    status = req("GET", "/api/auth/status").json()
    check("setup required on fresh install", status["setupRequired"], status)
    r = req("POST", "/api/auth/setup", {"email": "admin@example.com", "password": "supersecret1", "name": "Admin"})
    check("first admin setup", r.status == 200 and r.json()["user"]["isAdmin"], r.body[:200])
    check("setup is one-time", req("POST", "/api/auth/setup", {"email": "x@y.z", "password": "supersecret1", "name": "X"}).status == 400)
    check("wrong password rejected", req("POST", "/api/auth/login", {"email": "admin@example.com", "password": "nope"}).status == 401)
    login = req("POST", "/api/auth/login", {"email": "admin@example.com", "password": "supersecret1", "deviceType": "cli"})
    T = login.json()["accessToken"]
    check("login returns token + cookie", T and "ph_session=" in login.headers.get("set-cookie", ""))
    check("unauthenticated request rejected", req("GET", "/api/users/me").status == 401)
    check("users/me", req("GET", "/api/users/me", token=T).json()["email"] == "admin@example.com")
    key = req("POST", "/api/api-keys", {"name": "e2e"}, token=T).json()
    check("api key works", req("GET", "/api/users/me", headers={"x-api-key": key["secret"]}).status == 200)

    r = req("POST", "/api/admin/users", {"email": "bob@example.com", "password": "bobsecret1", "name": "Bob"}, token=T)
    bob = r.json()
    check("admin creates second user", r.status == 200 and bob["email"] == "bob@example.com", r.body[:200])
    TB = req("POST", "/api/auth/login", {"email": "bob@example.com", "password": "bobsecret1"}).json()["accessToken"]
    check("non-admin cannot list admin users", req("GET", "/api/admin/users", token=TB).status == 403)

    print("== upload")
    ids = {}
    for name in ["armstrong.jpg", "armstrong2.jpg", "armstrong3.jpg", "aldrin.jpg", "pano360.jpg", "clip.mov", "MVIMG_20240101.jpg"]:
        r = upload(T, media(name))
        ids[name] = r.json().get("id") if r.status in (200, 201) else None
        check(f"upload {name}", r.status == 201, r.body[:200])
    r = upload(T, media("live.mov"))
    ids["live.mov"] = r.json()["id"]
    r = upload(T, media("live.heic"), {"livePhotoVideoId": ids["live.mov"]})
    ids["live.heic"] = r.json()["id"]
    check("upload live photo (HEIC + MOV)", r.status == 201, r.body[:200])
    r = upload(T, media("armstrong.jpg"))
    check("re-upload is detected as duplicate", r.status == 200 and r.json()["status"] == "duplicate" and r.json()["id"] == ids["armstrong.jpg"])
    r = req("POST", "/api/assets/bulk-upload-check", {"assets": [
        {"id": "a", "checksum": sha1(media("armstrong.jpg"))}, {"id": "b", "checksum": "0" * 40}]}, token=T).json()
    check("bulk-upload-check", r["results"][0]["action"] == "reject" and r["results"][1]["action"] == "accept", r)
    r = req("POST", "/api/assets/exist", {"deviceId": "e2e", "deviceAssetIds": ["aldrin.jpg", "nope"]}, token=T).json()
    check("assets/exist", r["existingIds"] == ["aldrin.jpg"], r)
    check("unsupported file type rejected", upload(T, os.path.abspath(__file__)).status == 400)

    print("== external library")
    r = req("POST", "/api/libraries", {"name": "NAS", "importPaths": [A.ext_path], "watch": True}, token=T)
    lib = r.json()
    check("create external library", r.status == 200, r.body[:200])

    wait_jobs(T, "processing")
    time.sleep(1)
    wait_jobs(T, "follow-up jobs")

    print("== metadata & media")
    a = req("GET", f"/api/assets/{ids['armstrong.jpg']}", token=T).json()
    ex = a["exif"] or {}
    check("EXIF camera", ex.get("make") == "NASA" and ex.get("model") == "Hasselblad 500C", ex)
    check("EXIF exposure formatting", ex.get("exposureTime") == "1/250" and ex.get("fNumber") == 5.6, ex)
    check("GPS extracted", abs((ex.get("latitude") or 0) - 52.5163) < 1e-3, ex)
    check("reverse geocoding -> Berlin", ex.get("city") == "Berlin" and ex.get("country") == "Germany", ex)
    check("capture time used for timeline", a["localDateTime"].startswith("2019-10-26T14:30:00"), a["localDateTime"])
    check("timezone kept", ex.get("timeZone") == "+02:00" and ex.get("dateTimeOriginal", "").startswith("2019-10-26T12:30:00"), ex)
    check("keywords imported as tags", {"Mond", "Raumfahrt/Apollo"} <= {t["value"] for t in a["tags"]}, a["tags"])
    check("thumbhash present", bool(a["thumbhash"]))
    r = req("GET", f"/api/assets/{ids['armstrong.jpg']}/thumbnail", token=T)
    check("thumbnail served", r.status == 200 and r.body[:2] == b"\xff\xd8", r.status)
    r = req("GET", f"/api/assets/{ids['armstrong.jpg']}/thumbnail?size=preview", token=T)
    check("preview served", r.status == 200 and len(r.body) > 10000)
    r = req("GET", f"/api/assets/{ids['armstrong.jpg']}/original", token=T, headers={"Range": "bytes=0-99"})
    check("original download with Range", r.status == 206 and len(r.body) == 100, r.status)
    a3 = req("GET", f"/api/assets/{ids['armstrong3.jpg']}", token=T).json()
    check("EXIF orientation 6 swaps dimensions", a3["width"] == 1080 and a3["height"] == 1350, (a3["width"], a3["height"]))
    p = req("GET", f"/api/assets/{ids['pano360.jpg']}", token=T).json()
    check("360° photo detected", p["projectionType"] == "EQUIRECTANGULAR" and p["isPanorama"], p["projectionType"])
    v = req("GET", f"/api/assets/{ids['clip.mov']}", token=T).json()
    check("video duration + type", v["type"] == "VIDEO" and abs((v["duration"] or 0) - 3.0) < 0.2, v["duration"])
    r = req("GET", f"/api/assets/{ids['clip.mov']}/video", token=T, headers={"Range": "bytes=0-1023"})
    check("HEVC video transcoded and streamable", r.status == 206 and b"ftyp" in r.body[:16], r.status)
    live = req("GET", f"/api/assets/{ids['live.heic']}", token=T).json()
    check("HEIC decoded (thumbnail + size)", live["thumbhash"] and live["width"] == 960 and live["height"] == 1200, (live["width"], live["height"]))
    check("live photo linked", live["livePhotoVideoId"] == ids["live.mov"], live["livePhotoVideoId"])
    lv = req("GET", f"/api/assets/{ids['live.mov']}", token=T).json()
    check("live photo video hidden from timeline", lv["visibility"] == "hidden")
    mv = req("GET", f"/api/assets/{ids['MVIMG_20240101.jpg']}", token=T).json()
    check("motion photo video extracted", mv["isMotionPhoto"] and mv["livePhotoVideoId"], mv)
    if mv["livePhotoVideoId"]:
        r = req("GET", f"/api/assets/{mv['livePhotoVideoId']}/video", token=T)
        check("motion photo video playable", r.status == 200 and b"ftyp" in r.body[:16])

    print("== timeline / map / memories")
    buckets = req("GET", "/api/timeline/buckets", token=T).json()
    total = sum(b["count"] for b in buckets)
    check("timeline buckets (hidden parts excluded)", total == 10, buckets)
    b2019 = req("GET", "/api/timeline/bucket?timeBucket=2019-10-01", token=T).json()
    check("timeline bucket assets", [x["id"] for x in b2019] == [ids["armstrong.jpg"]], b2019)
    markers = req("GET", "/api/map/markers", token=T).json()
    check("map markers", len(markers) >= 3 and any(m["city"] == "Sydney" for m in markers), markers)
    mem = req("GET", "/api/memories?date=2026-10-26", token=T).json()
    years = sorted(m["year"] for m in mem)
    check("memories 'on this day'", years == [2018, 2019], [(m["year"], m["title"]) for m in mem])

    print("== machine learning")
    info = req("GET", "/api/server/info").json()
    check("ML features enabled", info["features"]["smartSearch"] and info["features"]["faces"], info)
    s = req("POST", "/api/search/smart", {"query": "an astronaut in a space suit"}, token=T).json()
    top = [x["originalFileName"] for x in s["assets"][:3]]
    check("smart search (CLIP) finds astronauts", any(n.startswith("armstrong") or n == "aldrin.jpg" for n in top[:1]), top)
    s = req("POST", "/api/search/smart", {"query": "colorful test pattern"}, token=T).json()
    check("smart search ranks test pattern", s["assets"][0]["originalFileName"] in ("pano360.jpg", "clip.mov", "library1.jpg"), [x["originalFileName"] for x in s["assets"][:3]])
    s = req("POST", "/api/search/metadata", {"text": "APOLLO"}, token=T).json()
    names = {x["originalFileName"] for x in s["assets"]}
    check("OCR text search", "armstrong.jpg" in names, names)
    s = req("POST", "/api/search/metadata", {"city": "berlin"}, token=T).json()
    check("metadata search by city", [x["id"] for x in s["assets"]] == [ids["armstrong.jpg"]], s)
    s = req("POST", "/api/search/metadata", {"is360": True}, token=T).json()
    check("metadata search 360°", [x["id"] for x in s["assets"]] == [ids["pano360.jpg"]])
    sugg = req("GET", "/api/search/suggestions?type=object", token=T).json()
    check("object recognition labels", len(sugg) > 0, sugg)
    explore = req("GET", "/api/search/explore", token=T).json()
    check("explore places/objects", explore["places"] and explore["objects"], {k: len(v) for k, v in explore.items()})
    people = req("GET", "/api/people", token=T).json()["people"]
    check("faces clustered into people", len(people) >= 1, people)
    if people:
        biggest = max(people, key=lambda p: p["faceCount"])
        check("person groups 3 photos", biggest["faceCount"] >= 3, biggest)
        r = req("PUT", f"/api/people/{biggest['id']}", {"name": "Neil"}, token=T)
        check("rename person", r.json()["name"] == "Neil")
        r = req("GET", f"/api/people/{biggest['id']}/thumbnail", token=T)
        check("person face thumbnail", r.status == 200 and r.body[:2] == b"\xff\xd8")
        pa = req("GET", f"/api/people/{biggest['id']}/assets", token=T).json()
        check("person assets", len(pa) >= 3)
        faces = req("GET", f"/api/faces?assetId={ids['armstrong.jpg']}", token=T).json()
        check("faces of asset with bounding box", faces and faces[0]["boundingBoxX2"] > faces[0]["boundingBoxX1"], faces)
        tl = req("GET", f"/api/timeline/buckets?personId={biggest['id']}", token=T).json()
        check("timeline filtered by person", sum(b["count"] for b in tl) == biggest["faceCount"])
    dups = req("GET", "/api/duplicates", token=T).json()
    dup_ids = [{x["id"] for x in g["assets"]} for g in dups]
    check("duplicate detection (same image, different file)", any({ids["armstrong.jpg"], ids["armstrong3.jpg"]} <= g for g in dup_ids), dup_ids)

    print("== external library results")
    lib = req("GET", "/api/libraries", token=T).json()[0]
    check("library indexed", lib["assetCount"] == 2, lib)

    print("== albums & sharing")
    r = req("POST", "/api/albums", {"albumName": "Mond", "assetIds": [ids["armstrong.jpg"], ids["aldrin.jpg"]],
                                     "albumUsers": [{"userId": bob["id"], "role": "viewer"}]}, token=T)
    album = r.json()
    check("create shared album", r.status == 200 and album["assetCount"] == 2 and album["shared"], r.body[:300])
    check("bob sees shared album", any(x["id"] == album["id"] for x in req("GET", "/api/albums?shared=true", token=TB).json()))
    check("bob can view album asset", req("GET", f"/api/assets/{ids['aldrin.jpg']}/thumbnail", token=TB).status == 200)
    check("bob cannot view non-shared asset", req("GET", f"/api/assets/{ids['pano360.jpg']}", token=TB).status == 404)
    check("viewer cannot add assets", req("PUT", f"/api/albums/{album['id']}/assets", {"ids": [ids["pano360.jpg"]]}, token=TB).status == 403)
    r = req("POST", f"/api/albums/{album['id']}/activities", {"type": "comment", "comment": "Toll!"}, token=TB)
    check("comment in shared album", r.status == 200)
    check("activities listed", len(req("GET", f"/api/albums/{album['id']}/activities", token=T).json()) == 1)

    r = req("POST", f"/api/partners/{bob['id']}", token=T)
    check("partner sharing", r.status == 200)
    req("PUT", f"/api/partners/{(req('GET', '/api/users/me', token=T).json())['id']}", {"inTimeline": True}, token=TB)
    tl = req("GET", "/api/timeline/buckets?withPartners=true", token=TB).json()
    check("partner assets in bob's timeline", sum(b["count"] for b in tl) == 10, tl)
    check("partner can read asset", req("GET", f"/api/assets/{ids['pano360.jpg']}", token=TB).status == 200)

    r = req("POST", "/api/shared-links", {"type": "ALBUM", "albumId": album["id"], "password": "geheim", "allowDownload": False}, token=T)
    link = r.json()
    check("create public link", r.status == 200 and link["url"].endswith("/s/" + link["key"]), r.body[:200])
    check("password required", req("GET", f"/api/shared-links/me?key={link['key']}").status == 401)
    st = req("POST", "/api/shared-links/me/login", {"key": link["key"], "password": "geheim"}).json()["shareToken"]
    pub = req("GET", f"/api/shared-links/me?key={link['key']}&shareToken={st}").json()
    check("public link lists album assets", len(pub["assets"]) == 2, pub)
    q = f"?key={link['key']}&shareToken={st}"
    check("public thumbnail", req("GET", f"/api/assets/{ids['aldrin.jpg']}/thumbnail{q}").status == 200)
    check("public download blocked when disabled", req("GET", f"/api/assets/{ids['aldrin.jpg']}/original{q}").status == 403)
    check("public link cannot access other assets", req("GET", f"/api/assets/{ids['pano360.jpg']}/thumbnail{q}").status == 404)

    print("== favorites, tags, archive, trash")
    req("PUT", f"/api/assets/{ids['pano360.jpg']}", {"isFavorite": True, "rating": 5, "description": "Rundum"}, token=T)
    fav = req("GET", "/api/timeline/buckets?isFavorite=true", token=T).json()
    check("favorites filter", sum(b["count"] for b in fav) == 1, fav)
    tag = req("POST", "/api/tags", {"value": "Reisen/Mond", "color": "#ff0000"}, token=T).json()
    req("PUT", f"/api/tags/{tag['id']}/assets", {"ids": [ids["aldrin.jpg"]]}, token=T)
    parent = [t for t in req("GET", "/api/tags", token=T).json() if t["value"] == "Reisen"][0]
    tl = req("GET", f"/api/timeline/buckets?tagId={parent['id']}", token=T).json()
    check("hierarchical tag filter", sum(b["count"] for b in tl) == 1, tl)
    req("PUT", "/api/assets", {"ids": [ids["armstrong2.jpg"]], "visibility": "archive"}, token=T)
    arch = req("GET", "/api/timeline/buckets?visibility=archive", token=T).json()
    check("archive", sum(b["count"] for b in arch) == 1)
    req("DELETE", "/api/assets", {"ids": [ids["live.heic"]]}, token=T)
    trash = req("GET", "/api/trash", token=T).json()
    check("trash (live photo moves with its video)", [x["id"] for x in trash] == [ids["live.heic"]])
    check("trashed video part too", req("GET", f"/api/assets/{ids['live.mov']}", token=T).json()["isTrashed"])
    req("POST", "/api/trash/restore", {"ids": [ids["live.heic"]]}, token=T)
    check("restore from trash", not req("GET", f"/api/assets/{ids['live.heic']}", token=T).json()["isTrashed"])

    print("== locked folder")
    check("unlock without pin fails", req("POST", "/api/auth/session/unlock", {"pin": "1234"}, token=T).status == 400)
    check("move to locked requires unlock", req("PUT", "/api/assets", {"ids": [ids["aldrin.jpg"]], "visibility": "locked"}, token=T).status == 403)
    check("set pin", req("PUT", "/api/users/me/pin", {"pin": "1234"}, token=T).status == 204)
    check("wrong pin rejected", req("POST", "/api/auth/session/unlock", {"pin": "9999"}, token=T).status == 401)
    check("unlock", req("POST", "/api/auth/session/unlock", {"pin": "1234"}, token=T).status == 200)
    check("move to locked", req("PUT", "/api/assets", {"ids": [ids["aldrin.jpg"]], "visibility": "locked"}, token=T).status == 204)
    locked = req("GET", "/api/timeline/buckets?visibility=locked", token=T).json()
    check("locked timeline", sum(b["count"] for b in locked) == 1)
    check("locked asset left the album", req("GET", f"/api/albums/{album['id']}", token=T).json()["assetCount"] == 1)
    req("POST", "/api/auth/session/lock", token=T)
    check("locked asset hidden after lock", req("GET", f"/api/assets/{ids['aldrin.jpg']}/thumbnail", token=T).status == 403)
    check("locked timeline requires unlock", req("GET", "/api/timeline/buckets?visibility=locked", token=T).status == 403)
    check("locked assets excluded from search", ids["aldrin.jpg"] not in {x["id"] for x in req("POST", "/api/search/metadata", {}, token=T).json()["assets"]})
    check("partner cannot see locked asset", req("GET", f"/api/assets/{ids['aldrin.jpg']}", token=TB).status in (403, 404))

    print("== editor")
    before = req("GET", f"/api/assets/{ids['pano360.jpg']}", token=T).json()
    r = req("PUT", f"/api/assets/{ids['pano360.jpg']}/edits", {"rotate": 90, "crop": {"x": 0, "y": 0, "width": 1, "height": 0.5}, "filter": "bw", "brightness": 10}, token=T)
    e = r.json()
    check("edit applied (non-destructive)", r.status == 200 and e["hasEdits"] and e["width"] == 1000 and e["height"] == 1000, (r.status, e.get("width"), e.get("height")))
    check("original untouched", req("GET", f"/api/assets/{ids['pano360.jpg']}/original", token=T).body[:2] == b"\xff\xd8" and
          e["checksum"] == before["checksum"])
    r = req("GET", f"/api/assets/{ids['pano360.jpg']}/original?edited=true", token=T)
    check("download edited version", r.status == 200 and "edited" in r.headers.get("content-disposition", ""))
    r = req("PUT", f"/api/assets/{ids['armstrong2.jpg']}/edits", {"rotate": 180, "saveAsCopy": True}, token=T)
    check("save edit as copy", r.status == 200 and r.json()["id"] != ids["armstrong2.jpg"], r.body[:200])
    r = req("DELETE", f"/api/assets/{ids['pano360.jpg']}/edits", token=T)
    check("revert edits", not r.json()["hasEdits"])
    check("invalid edit rejected", req("PUT", f"/api/assets/{ids['pano360.jpg']}/edits", {"rotate": 45}, token=T).status == 400)

    print("== cast")
    c = req("POST", f"/api/assets/{ids['clip.mov']}/cast-url", token=T).json()
    r = req("GET", c["url"], headers={"Range": "bytes=0-99"})
    check("cast URL works without login", r.status == 206, r.status)
    other = c["url"].replace(ids["clip.mov"], ids["pano360.jpg"])
    check("cast token bound to its asset", req("GET", other).status == 401)

    print("== sync / admin / backup / metrics")
    d = req("GET", "/api/sync/delta?updatedAfter=2000-01-01T00:00:00Z", token=T).json()
    check("sync delta", len(d["upserted"]) >= 12 and not d["hasMore"])
    stats = req("GET", "/api/server/statistics", token=T).json()
    check("server statistics", stats["photos"] >= 9, stats)
    cfg = req("PUT", "/api/admin/config", {"trash": {"days": 7}}, token=T).json()
    check("system config update", cfg["trash"]["days"] == 7 and cfg["ml"]["faceMinFaces"] == 3)
    check("invalid cron rejected", req("PUT", "/api/admin/config", {"backup": {"cron": "nope"}}, token=T).status == 400)
    check("trigger DB backup", req("POST", "/api/admin/backups", token=T).status == 202)
    wait_jobs(T, "backup")
    backups = req("GET", "/api/admin/backups", token=T).json()
    check("DB backup written", backups and backups[0]["size"] > 1000, backups)
    if backups:
        r = req("GET", f"/api/admin/backups/{backups[0]['name']}", token=T)
        check("backup download is gzip", r.body[:2] == b"\x1f\x8b")
    check("job command", req("POST", "/api/jobs/thumbnails", {"command": "pause"}, token=T).status == 204 and
          req("GET", "/api/jobs", token=T).json()["thumbnails"]["paused"])
    req("POST", "/api/jobs/thumbnails", {"command": "resume"}, token=T)
    m = req("GET", A.metrics).body.decode()
    check("prometheus metrics", "pixelhort_http_requests_total" in m and "pixelhort_jobs_completed_total" in m and "pixelhort_assets" in m)

    print("== permanent delete")
    req("DELETE", "/api/assets", {"ids": [ids["armstrong2.jpg"]], "force": True}, token=T)
    check("permanent delete", req("GET", f"/api/assets/{ids['armstrong2.jpg']}", token=T).status == 404)
    d = req("GET", "/api/sync/delta?updatedAfter=2000-01-01T00:00:00Z", token=T).json()
    check("deletion appears in sync delta", ids["armstrong2.jpg"] in d["deleted"])

    passed = sum(1 for _, ok in RESULTS if ok)
    print(f"\n{passed}/{len(RESULTS)} checks passed")
    sys.exit(0 if passed == len(RESULTS) else 1)


if __name__ == "__main__":
    main()
