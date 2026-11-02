"""Supervised release probes: synthetic broken inputs, originals never modified."""
import hashlib
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import tomllib


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    app = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser()
    parser.add_argument("--player", type=Path, default=app / "EnigmaxCinePlayer.exe")
    player = parser.parse_args().player.resolve(strict=True)
    version = tomllib.loads((app / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    out = app / f"verificacion/{version}"
    out.mkdir(exist_ok=True)
    config_before = {p.name: digest(p) for p in (app / "config").glob("*") if p.is_file()}
    error = player.parent / "ultimo-error.txt"
    former_error = error.read_bytes() if error.exists() else None
    rows = []
    try:
        with tempfile.TemporaryDirectory(prefix="fault-", dir=out) as temporary:
            temp = Path(temporary)
            fixtures = app / "verificacion/muestras"
            cases = [(fixtures / "srt_audio.mkv", True), (fixtures / "ass_audio.mkv", True),
                     (fixtures / "uhd_hdr10.mkv", True)]
            for name, content in [("empty.mkv", b""), ("truncated.mkv", (fixtures / "srt_audio.mkv").read_bytes()[:32]),
                                  ("invalid.mp4", b"invalid MP4 header"), ("invalid.iso", bytes(32768))]:
                path = temp / name
                path.write_bytes(content)
                cases.append((path, False))
            for index, (source, valid) in enumerate(cases):
                env = dict(os.environ, ENIGMAX_PROBE_PLAY_SECONDS="1")
                start = time.monotonic()
                try:
                    result = subprocess.run([str(player), "--probe", str(source),
                                             "original", str(temp / f"case-{index}.png")], env=env,
                                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                            timeout=15, creationflags=subprocess.CREATE_NO_WINDOW)
                    row = dict(file=source.name, valid=valid, code=result.returncode,
                               seconds=round(time.monotonic() - start, 3))
                    row["passed"] = result.returncode == (0 if valid else 1)
                except subprocess.TimeoutExpired:
                    row = dict(file=source.name, valid=valid, timeout=True, passed=False)
                rows.append(row)
    finally:
        if former_error is None:
            error.unlink(missing_ok=True)
        else:
            error.write_bytes(former_error)
    config_after = {p.name: digest(p) for p in (app / "config").glob("*") if p.is_file()}
    report = dict(cases=rows, config_unchanged=config_before == config_after)
    (out / "fault-probes.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if all(row["passed"] for row in rows) and report["config_unchanged"] else 1)
