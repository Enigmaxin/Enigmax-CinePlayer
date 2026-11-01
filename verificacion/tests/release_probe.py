"""Supervise the current release executable, including normal teardown per case."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


if __name__ == "__main__":
    app = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser()
    parser.add_argument("--player", type=Path, required=True)
    args = parser.parse_args()
    player = args.player.resolve(strict=True)
    fixtures = app / "verificacion/muestras"
    out = app / "verificacion/0.2.23/release"
    out.mkdir(parents=True, exist_ok=True)
    config = lambda: {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in (app / "config").iterdir() if p.is_file()}
    before = config()
    cases = [
        ("fullsbs", fixtures / "ass3d-tagged-fullsbs.mkv", "fsbs-both"),
        ("hsbs", fixtures / "ass3d-hsbs.mkv", "hsbs-both"),
        ("fullou", fixtures / "ass3d-tagged-fullou.mkv", "fou-both"),
        ("hou", fixtures / "ass3d-hou.mkv", "hou-both"),
        ("anaglyph", fixtures / "ass3d-fullsbs.mkv", "fsbs-ana"),
        ("anamorphic", fixtures / "ass3d-fullsbs.mkv", "fsbs-sbs2160"),
        ("framepack", fixtures / "ass3d-fullsbs.mkv", "fsbs-framepack"),
        ("pgs", fixtures / "depth-qa-pgs.mkv", "fsbs-ana"),
    ]
    hellcat = Path(r"E:\Hellcat [1040p] [HEVC]\Hellcat [1040p] [HEVC].mkv")
    if hellcat.is_file():
        cases.extend((f"hellcat-{n}", hellcat, "original") for n in range(3))
    rows = []
    for name, source, mode in cases:
        start = time.monotonic()
        env = {k: v for k, v in os.environ.items() if not k.startswith("ENIGMAX_PROBE_")}
        env.update(ENIGMAX_PROBE_PLAY_SECONDS="2", ENIGMAX_PROBE_CONTROLS="1")
        capture = out / f"{name}.png"
        try:
            result = subprocess.run([str(player), "--probe", str(source), mode, str(capture)],
                                    env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, timeout=50,
                                    creationflags=subprocess.CREATE_NO_WINDOW)
            row = dict(case=name, mode=mode, exit=result.returncode,
                       seconds=round(time.monotonic()-start, 3),
                       passed=result.returncode == 0 and capture.is_file())
        except subprocess.TimeoutExpired:
            row = dict(case=name, passed=False, timeout=True)
        rows.append(row)
        print(json.dumps(row), flush=True)
    report = dict(cases=rows, config_unchanged=before == config())
    (out / "results.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    raise SystemExit(0 if report["config_unchanged"] and all(r["passed"] for r in rows) else 1)
