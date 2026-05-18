#!/usr/bin/env python3
"""
Minimal freight registry server for integration testing.

Implements only the endpoints freight actually calls:
  GET  /api/v1/packages/{name}                  — metadata + version list
  GET  /api/v1/packages/{name}/{ver}/download   — source/prebuilt tarball
  GET  /api/v1/packages/{name}/{ver}/prebuilts  — empty (we serve source only)
  GET  /api/v1/search?q=...                     — package search

Package tarballs live in  data/packages/{name}/{version}.tar.gz
relative to this script's directory.
"""

import hashlib
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from urllib.parse import urlparse, parse_qs

PORT    = int(os.environ.get("REGISTRY_PORT", 7878))
DATA    = Path(__file__).parent / "data" / "packages"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def package_versions(name: str) -> list[dict]:
    pkg_dir = DATA / name
    if not pkg_dir.is_dir():
        return []
    versions = []
    for tarball in sorted(pkg_dir.glob("*.tar.gz")):
        version = tarball.stem
        if version.endswith(".tar"):        # handle .tar.gz → stem is still ".tar"
            version = Path(version).stem
        checksum = sha256_file(tarball)
        versions.append({
            "version":          version,
            "checksum":         checksum,
            "download_url":     None,       # client builds it from the standard path
            "prebuilt_triples": [],
            "dependencies":     {},
        })
    return versions


def latest_version(versions: list[dict]) -> str:
    if not versions:
        return "0.0.0"
    return versions[-1]["version"]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[registry] {fmt % args}", file=sys.stderr)

    def send_json(self, code: int, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_bytes(self, code: int, data: bytes, checksum: str | None = None):
        self.send_response(code)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(data)))
        if checksum:
            self.send_header("X-Checksum-SHA256", checksum)
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        parsed = urlparse(self.path)
        parts  = parsed.path.strip("/").split("/")

        # GET /api/v1/packages/{name}
        if parts[:3] == ["api", "v1", "packages"] and len(parts) == 4:
            name     = parts[3]
            versions = package_versions(name)
            if not versions:
                self.send_json(404, {"error": f"package '{name}' not found"})
                return
            self.send_json(200, {
                "name":        name,
                "description": f"Test package: {name}",
                "latest":      latest_version(versions),
                "versions":    versions,
            })
            return

        # GET /api/v1/packages/{name}/{ver}/download
        if parts[:3] == ["api", "v1", "packages"] and len(parts) == 6 and parts[5] == "download":
            name, version = parts[3], parts[4]
            tarball = DATA / name / f"{version}.tar.gz"
            if not tarball.exists():
                self.send_json(404, {"error": f"{name}@{version} not found"})
                return
            data     = tarball.read_bytes()
            checksum = sha256_file(tarball)
            self.send_bytes(200, data, checksum)
            return

        # GET /api/v1/packages/{name}/{ver}/prebuilts
        if parts[:3] == ["api", "v1", "packages"] and len(parts) == 6 and parts[5] == "prebuilts":
            self.send_json(200, {"prebuilts": []})
            return

        # GET /api/v1/search?q=...
        if parts[:3] == ["api", "v1", "search"]:
            qs    = parse_qs(parsed.query)
            query = (qs.get("q") or [""])[0].lower()
            pkgs  = []
            if DATA.is_dir():
                for pkg_dir in sorted(DATA.iterdir()):
                    if not pkg_dir.is_dir(): continue
                    name = pkg_dir.name
                    if query and query not in name: continue
                    versions = package_versions(name)
                    if not versions: continue
                    pkgs.append({
                        "name":        name,
                        "description": f"Test package: {name}",
                        "latest":      latest_version(versions),
                        "versions":    versions,
                    })
            self.send_json(200, {"packages": pkgs})
            return

        self.send_json(404, {"error": "not found"})


if __name__ == "__main__":
    DATA.mkdir(parents=True, exist_ok=True)
    server = HTTPServer(("127.0.0.1", PORT), Handler)
    print(f"[registry] listening on http://127.0.0.1:{PORT}  (data: {DATA})", file=sys.stderr)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
