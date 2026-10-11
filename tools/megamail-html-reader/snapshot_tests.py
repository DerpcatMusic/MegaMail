#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Actual isolated WebKit renders of fictional mail, using an already built package."""
import argparse
import base64
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile

from PIL import Image
from fixture_request import request


def run(helper, obj, accepted=True):
    payload = json.dumps(obj).encode()
    child = subprocess.Popen([str(helper), "--snapshot"], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True)
    try:
        stdout, stderr = child.communicate(payload, timeout=20)
    except subprocess.TimeoutExpired:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
        child.communicate(timeout=3)
        raise AssertionError("isolated snapshot timed out")
    events = [json.loads(line) for line in stdout.splitlines()]
    if not accepted:
        assert child.returncode != 0 and not any(e["event"] == "frame" for e in events)
        return None
    assert child.returncode == 0, [e.get("code", e["event"]) for e in events]
    assert [e["event"] for e in events] in (
        ["ready", "frame-links", "frame", "closed"],
        ["ready", "frame-clipped", "frame-links", "frame", "closed"],
    )
    ready = events[0]
    assert ready["policy"] == "v1:extension+sandbox+ephemeral+no-network"
    assert len(ready["token"]) == 32 and all(e["token"] == ready["token"] for e in events)
    links = json.loads(next(e["links"] for e in events if e["event"] == "frame-links"))
    png = base64.b64decode(next(e["png"] for e in events if e["event"] == "frame"), validate=True)
    image = Image.open(io.BytesIO(png)).convert("RGB")
    assert image.width == obj.get("viewportWidth", 640) and 1 <= image.height <= 8192
    assert len(links) <= 256
    for link in links:
        assert link["url"].startswith(("https://", "http://"))
        assert 0 <= link["x"] < image.width and 0 <= link["y"] < image.height
        assert 0 < link["w"] <= image.width-link["x"]
        assert 0 < link["h"] <= image.height-link["y"]
    return image, links, any(e["event"] == "frame-clipped" for e in events)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--helper", required=True, type=Path)
    parser.add_argument("--artifacts", type=Path)
    args = parser.parse_args()
    helper = args.helper.resolve(strict=True)
    wide, links, clipped = run(helper, request())
    assert not clipped and len(links) == 1 and links[0]["label"] == "Read the notes"
    assert wide.getpixel((8, 8)) == (233, 240, 237)
    assert wide.getpixel((wide.width-8, 8)) == (245, 241, 232)
    narrow_request = request(); narrow_request["viewportWidth"] = 400
    narrow, links, clipped = run(helper, narrow_request)
    assert not clipped and len(links) == 1 and narrow.height > wide.height
    assert narrow.getpixel((8, 8)) == (233, 240, 237)
    assert narrow.getpixel((8, narrow.height-8)) == (245, 241, 232)
    hidden = request()
    hidden["html"] = hidden["html"].replace("<body>", '<body><div style="display:none">Hidden duplicate preheader</div>')
    hidden_image, _, _ = run(helper, hidden)
    assert hidden_image.size == wide.size and hidden_image.tobytes() == wide.tobytes()
    for name in ("hostile", "parser-confusion"):
        _, links, _ = run(helper, request(name))
        assert all("user:secret" not in link["url"] for link in links)
    long = request(); long["html"] = '<html><body style="margin:0"><a href="https://example.test/top">Top</a><div style="height:9000px"></div><a href="https://example.test/bottom">Bottom</a></body></html>'
    image, links, clipped = run(helper, long)
    assert image.height == 8192 and clipped and [link["label"] for link in links] == ["Top"]
    for width in (0, 319, 1281, 400.5, "640"):
        invalid = request(); invalid["viewportWidth"] = width
        run(helper, invalid, accepted=False)
    with tempfile.TemporaryDirectory(prefix="megamail-missing-extension-") as directory:
        copied = Path(directory) / helper.name
        copied.write_bytes(helper.read_bytes()); copied.chmod(0o755)
        run(copied, request(), accepted=False)
    if args.artifacts:
        args.artifacts.mkdir(parents=True, exist_ok=True)
        wide.save(args.artifacts / "styled-wide.png")
        narrow.save(args.artifacts / "styled-narrow.png")
    print(json.dumps({"isolated_WebKit_fixtures":"pass", "responsive_CSS":True,
                      "hidden_preheader":True, "bounded_clipping":True,
                      "safe_link_rectangles":True, "missing_policy_fails_closed":True}))


if __name__ == "__main__":
    main()
