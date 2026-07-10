#!/usr/bin/env python3
"""Verify tracked word-asset hashes and optional release copies."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ASSETS = {
    "wordlist": ROOT / "wordlist.json.gz",
    "dictionary": ROOT / "dictionary.json.gz",
}


def sha256_prefix(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def check_canonical_dictionary() -> None:
    with gzip.open(ASSETS["dictionary"], "rb") as source:
        payload = source.read()
    parsed = json.loads(payload)
    canonical = (json.dumps(parsed, separators=(",", ":"), sort_keys=True) + "\n").encode()
    if payload != canonical:
        raise SystemExit("dictionary.json.gz does not contain canonical compact JSON")
    expected = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=expected, mtime=0) as output:
        output.write(canonical)
    if ASSETS["dictionary"].read_bytes() != expected.getvalue():
        raise SystemExit("dictionary.json.gz is not deterministically compressed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=Path, help="also verify copied release assets")
    args = parser.parse_args()

    check_canonical_dictionary()
    manifest_path = ROOT / "word-assets.json"
    manifest = json.loads(manifest_path.read_text())
    expected_version = "sha256-{}-{}".format(
        sha256_prefix(ASSETS["wordlist"]),
        sha256_prefix(ASSETS["dictionary"]),
    )
    if manifest.get("version") != expected_version:
        raise SystemExit(
            f"word-assets.json version is {manifest.get('version')!r}; expected {expected_version!r}"
        )

    fallback = f'version: "{expected_version}"'
    if fallback not in (ROOT / "index.html").read_text():
        raise SystemExit("index.html word-asset fallback version is stale")

    for name, source in ASSETS.items():
        expected_url = f"/{source.name}"
        if manifest.get(name) != expected_url:
            raise SystemExit(f"word-assets.json {name} URL must be {expected_url!r}")

    if args.dist is not None:
        dist = args.dist.resolve()
        for source in [manifest_path, *ASSETS.values()]:
            copied = dist / source.name
            if not copied.is_file() or copied.read_bytes() != source.read_bytes():
                raise SystemExit(f"release asset is missing or stale: {copied}")

    print(f"word assets verified: {expected_version}")


if __name__ == "__main__":
    main()
