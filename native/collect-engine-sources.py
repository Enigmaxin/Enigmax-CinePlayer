"""Collect official MSYS2 source archives matching installed signed packages.

Requires Python 3.14 (zstd tar support). Archives remain compressed; no package
script is executed. A source PKGBUILD must match the SHA recorded in the signed
binary's BUILDINFO. This collector does not assert bit-for-bit reproducibility.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import tarfile
import time
import urllib.error
import urllib.request
from pathlib import Path


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fields(path: Path) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {}
    current = ""
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("%") and line.endswith("%"):
            current = line.strip("%")
            result[current] = []
        elif line and current:
            result[current].append(line)
    return result


def tar_texts(path: Path, names: set[str]) -> dict[str, bytes]:
    found: dict[str, bytes] = {}
    with tarfile.open(path, "r|*") as archive:
        for entry in archive:
            name = entry.name.rsplit("/", 1)[-1]
            if name in names and entry.isfile():
                if entry.size > 2 * 1024 * 1024:
                    raise ValueError(f"Oversized metadata: {path}: {entry.name}")
                stream = archive.extractfile(entry)
                if stream is None:
                    raise ValueError(f"Missing metadata stream: {path}")
                found[name] = stream.read()
                if names <= found.keys():
                    break
    return found


def download(url: str, target: Path) -> None:
    if target.is_file():
        return
    partial = target.with_name(target.name + ".part")
    for attempt in range(4):
        try:
            request = urllib.request.Request(url, headers={"User-Agent": "Enigmax-CinePlayer-Source-Collector/1"})
            with urllib.request.urlopen(request, timeout=40) as response, partial.open("wb") as out:
                total = 0
                while data := response.read(256 * 1024):
                    total += len(data)
                    if total > 768 * 1024 * 1024:
                        raise ValueError(f"Source archive exceeds limit: {url}")
                    out.write(data)
            partial.replace(target)
            return
        except urllib.error.HTTPError as error:
            if error.code not in (429, 500, 502, 503, 504) or attempt == 3:
                raise
            wait = min(30, max(5, int(error.headers.get("Retry-After", "5"))))
        except (TimeoutError, urllib.error.URLError):
            if attempt == 3:
                raise
            wait = 5
        time.sleep(wait)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--msys", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    destination = args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    installed: dict[str, dict[str, list[str]]] = {}
    providers: dict[str, str] = {}
    for desc in (args.msys / "var/lib/pacman/local").glob("*/desc"):
        data = fields(desc)
        if "NAME" not in data:
            continue
        name = data["NAME"][0]
        installed[name] = data
        providers[name] = name
        for provide in data.get("PROVIDES", []):
            providers[re.split(r"[<>=]", provide)[0]] = name
    roots = [
        "ffmpeg", "libplacebo", "libass", "libbluray", "lua51", "lcms2",
        "libarchive", "uchardet", "shaderc", "spirv-cross", "vulkan-headers",
        "amf-headers", "ffnvcodec-headers",
    ]
    pending = ["mingw-w64-ucrt-x86_64-" + root for root in roots]
    selected: set[str] = set()
    while pending:
        requested = re.split(r"[<>=]", pending.pop())[0]
        if requested not in providers:
            raise ValueError(f"Dependency not installed: {requested}")
        name = providers[requested]
        if name in selected:
            continue
        selected.add(name)
        pending.extend(installed[name].get("DEPENDS", []))
    cache = args.msys / "var/cache/pacman/pkg"
    records: dict[str, dict] = {}
    for name in sorted(selected):
        data = installed[name]
        if "pgp" not in data.get("VALIDATION", []):
            raise ValueError(f"Package was not PGP-validated: {name}")
        version = data["VERSION"][0]
        filename_version = version.replace(":", "~")
        files = list(cache.glob(f"{name}-{filename_version}-*.pkg.tar.zst"))
        if len(files) != 1:
            raise ValueError(f"Expected one cached signed binary package for {name}: {files}")
        metadata = tar_texts(files[0], {".BUILDINFO", ".PKGINFO"})
        if ".BUILDINFO" not in metadata:
            raise ValueError(f"No BUILDINFO for {name}")
        build = metadata[".BUILDINFO"].decode("utf-8")
        expected = re.search(r"(?m)^pkgbuild_sha256sum = ([a-f0-9]{64})$", build)
        if not expected:
            raise ValueError(f"No PKGBUILD hash for {name}")
        base = data["BASE"][0]
        source_version = re.sub(r"^\d+[:~]", "", version)
        source_name = f"{base}-{source_version}.src.tar.zst"
        if source_name in records:
            records[source_name]["binaryPackages"].append(name)
            continue
        url = f"https://repo.msys2.org/mingw/sources/{source_name}"
        archive_path = destination / source_name
        print(f"Collecting: {source_name}", flush=True)
        try:
            download(url, archive_path)
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
            index_path = destination / "source-index.html"
            download("https://repo.msys2.org/mingw/sources/", index_path)
            names = re.findall(r'href="([^"/]+\.src\.tar\.(?:zst|xz|gz))"', index_path.read_text(encoding="utf-8"))
            candidates = []
            for candidate in names:
                if not candidate.startswith(base + "-"):
                    continue
                candidate_version = re.sub(r"\.src\.tar\.(?:zst|xz|gz)$", "", candidate[len(base) + 1:])
                if re.sub(r"^\d+[:~]", "", candidate_version) == source_version:
                    candidates.append(candidate)
            if len(candidates) != 1:
                raise ValueError(f"Source archive missing or ambiguous for {name} {version}: {url}") from error
            source_name = candidates[0]
            url = f"https://repo.msys2.org/mingw/sources/{source_name}"
            archive_path = destination / source_name
            download(url, archive_path)
        contents = tar_texts(archive_path, {"PKGBUILD", ".SRCINFO"})
        actual = hashlib.sha256(contents.get("PKGBUILD", b"")).hexdigest()
        if actual != expected[1]:
            raise ValueError(f"Source recipe does not match signed binary package: {name}")
        if ".SRCINFO" not in contents:
            raise ValueError(f"No .SRCINFO in {source_name}")
        metadata_dir = destination / "metadata" / f"{base}-{source_version}"
        metadata_dir.mkdir(parents=True, exist_ok=True)
        for entry, value in metadata.items():
            (metadata_dir / entry.lstrip(".")).write_bytes(value)
        for entry, value in contents.items():
            (metadata_dir / entry.lstrip(".")).write_bytes(value)
        records[source_name] = {
            "archive": source_name, "url": url, "sha256": sha256(archive_path),
            "bytes": archive_path.stat().st_size, "pkgbuildSha256": actual,
            "binaryPackages": [name], "installedVersion": version,
            "verification": "PKGBUILD matches BUILDINFO of PGP-validated binary package",
        }
        (destination / "sources.lock.json").write_text(
            json.dumps({"format": 1, "packages": list(records.values())}, indent=2), encoding="utf-8"
        )
        print(f"Source verified: {source_name} ({archive_path.stat().st_size // 1024} KiB)", flush=True)
    print(f"Collected {len(records)} source archives for {len(selected)} signed binary packages.", flush=True)


if __name__ == "__main__":
    main()
