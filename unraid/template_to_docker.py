#!/usr/bin/env python3
"""Turns an Unraid template into the `docker run` arguments Unraid would use.

python unraid/template_to_docker.py unraid/pixelhort.xml [--image local:tag] [--set NAME=VALUE ...]
Prints a JSON list of arguments (used by the template test and handy for debugging).
"""
import argparse
import json
import xml.etree.ElementTree as ET

p = argparse.ArgumentParser()
p.add_argument("template")
p.add_argument("--image", help="override the repository")
p.add_argument("--set", action="append", default=[], help="override a Config value by Target")
a = p.parse_args()

root = ET.parse(a.template).getroot()
overrides = dict(s.split("=", 1) for s in a.set)
args = ["run", "-d", "--name", root.findtext("Name"), "--net", root.findtext("Network")]
for c in root.findall("Config"):
    target, kind = c.get("Target"), c.get("Type")
    value = overrides.get(target, (c.text or "").strip())
    if not value:
        continue
    if kind == "Variable":
        args += ["-e", f"{target}={value}"]
    elif kind == "Port":
        args += ["-p", f"{value}:{target}/{c.get('Mode') or 'tcp'}"]
    elif kind == "Path":
        args += ["-v", f"{value}:{target}:{c.get('Mode') or 'rw'}"]
    elif kind == "Device":
        args += ["--device", value]
extra = (root.findtext("ExtraParams") or "").split()
args += extra + [a.image or root.findtext("Repository")]
print(json.dumps(args))
