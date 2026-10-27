"""Stage only the recursively imported engine DLLs, recording their provenance.

No player files are changed. Missing imports or ambiguous package ownership fail
the staging operation. Windows system/API-set imports are never redistributed.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
from collections import deque
from pathlib import Path


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


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--prefix", type=Path, required=True)
    p.add_argument("--msys", type=Path, required=True)
    p.add_argument("--bridge", type=Path, required=True)
    p.add_argument("--objdump", type=Path, required=True)
    p.add_argument("--destination", type=Path, required=True)
    args = p.parse_args()
    destination = args.destination.resolve()
    if destination.exists():
        raise ValueError(f"Staging directory must be new: {destination}")
    own_bin = (args.prefix / "bin").resolve()
    system_bin = (args.msys / "ucrt64/bin").resolve()
    win_bin = Path(os.environ["SystemRoot"]) / "System32"
    module_map = {f.name.lower(): f for f in system_bin.glob("*.dll")}
    module_map.update({f.name.lower(): f for f in own_bin.glob("*.dll")})
    bridge = args.bridge.resolve()
    if not bridge.is_file():
        raise ValueError("MVC bridge is missing")
    module_map["cine_mvc.dll"] = bridge
    ownership: dict[str, dict] = {}
    for desc in (args.msys / "var/lib/pacman/local").glob("*/desc"):
        data = fields(desc)
        listing = desc.with_name("files")
        if not listing.is_file():
            continue
        for entry in fields(listing).get("FILES", []):
            if entry.startswith("ucrt64/bin/") and entry.lower().endswith(".dll"):
                key = entry.rsplit("/", 1)[-1].lower()
                if key in ownership:
                    raise ValueError(f"Ambiguous DLL ownership: {key}")
                ownership[key] = {
                    "package": data["NAME"][0], "version": data["VERSION"][0],
                    "base": data["BASE"][0], "licenses": data.get("LICENSE", []),
                    "files": fields(listing).get("FILES", []),
                }
    queue = deque(["libmpv-2.dll", "cine_mvc.dll"])
    seen: set[str] = set()
    modules = []
    system_imports: set[str] = set()
    while queue:
        name = queue.popleft().lower()
        if name in seen:
            continue
        seen.add(name)
        if not re.fullmatch(r"[a-z0-9_.+\-]+\.dll", name):
            raise ValueError(f"Unexpected DLL import: {name}")
        source = module_map.get(name)
        if source is None:
            if name.startswith(("api-ms-", "ext-ms-")) or (win_bin / name).is_file():
                system_imports.add(name)
                continue
            raise ValueError(f"Unresolved DLL: {name}")
        raw = subprocess.check_output([str(args.objdump.resolve()), "-p", str(source)], text=True, errors="replace")
        if "pei-x86-64" not in raw:
            raise ValueError(f"Not a Windows x64 PE DLL: {source}")
        imports = re.findall(r"DLL Name:\s*(\S+)", raw)
        queue.extend(imports)
        relative = "libmpv-2.dll" if name == "libmpv-2.dll" else "runtime/" + source.name
        entry = {"path": relative, "bytes": source.stat().st_size, "sha256": digest(source), "imports": imports}
        if source.parent == own_bin:
            entry["provider"] = "own-source-build"
        elif name == "cine_mvc.dll":
            entry["provider"] = "own-mvc-udf-source-build"
        else:
            if name not in ownership:
                raise ValueError(f"No signed package owner: {name}")
            entry.update({k: ownership[name][k] for k in ("package", "version", "base", "licenses")})
            entry["provider"] = "signature-verified-msys2-package"
        modules.append((source, entry))
    # No writes until the complete import graph has been resolved.
    (destination / "runtime").mkdir(parents=True)
    for source, entry in modules:
        shutil.copyfile(source, destination / entry["path"])
    (destination / "engine-manifest.json").write_text(json.dumps({
        "format": 1, "modules": [entry for _, entry in modules],
        "systemImports": sorted(system_imports),
        "mpvCommit": "e471496c56f0f49c133bd88b50aed62b845f1909",
        "ffmpegCommit": "6882969ce9fd2aa6aa1d0008b2bfe1c838aac11d",
    }, indent=2), encoding="utf-8")
    notices = []
    copied_notices: set[str] = set()
    for source, entry in modules:
        owner = ownership.get(source.name.lower()) if entry["provider"] == "signature-verified-msys2-package" else None
        if owner is None:
            continue
        notices.append(f"\nComponent: {owner['package']} {owner['version']}\nLicense identifiers: {', '.join(owner['licenses'])}\n")
        for item in owner["files"]:
            if "/share/licenses/" not in item or item.endswith("/") or item in copied_notices:
                continue
            path = args.msys / item
            if path.is_file() and path.stat().st_size < 2 * 1024 * 1024:
                notices.append(f"\n--- {item} ---\n" + path.read_text(encoding="utf-8", errors="replace"))
                copied_notices.add(item)
    (destination / "third-party-runtime.txt").write_text("\n".join(notices), encoding="utf-8")
    total = sum(entry["bytes"] for _, entry in modules)
    print(f"Staged {len(modules)} DLLs ({total / 1024**2:.1f} MiB); Windows system DLLs excluded.")


if __name__ == "__main__":
    main()
