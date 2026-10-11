#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Emit only fictional, repository-owned reader fixtures; never load a mailbox."""
import argparse
import json
from pathlib import Path

FIXTURES = Path(__file__).resolve().parents[2] / "docs/fixtures/html-reader/batch2"
PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGMwmFAAAAIkATGtLlwIAAAAAElFTkSuQmCC"


def request(name="authored"):
    return {
        "version": 1,
        "html": (FIXTURES / (name + ".html")).read_text(encoding="utf-8"),
        "plainText": "Fictional fixture. Select this sentence and copy it. Plain fallback remains available.",
        "title": "Synthetic reader fixture",
        "resources": [{"cid": "logo@fixture.invalid", "mime": "image/png", "base64": PNG}],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", choices=["authored", "hostile", "parser-confusion"], default="authored")
    print(json.dumps(request(parser.parse_args().fixture), ensure_ascii=False))
