"""Fetch the three pinned Windows-adapted PingFang faces without changing font bytes."""

import argparse
import hashlib
import subprocess
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

COMMIT = "8444592e20420a9d412a851542f00f7ee9c3144f"
REPO = "ACT-02/PingFang-for-Windows"
FILES = {
    "PingFangSC-Regular.otf": (13144336, "0fea38f2c15e775876ed81cace5028b26c90e67a"),
    "PingFangSC-Medium.otf": (13010568, "79c1618dbae3215fd9b5da451e6f8b4d3ac2a5ef"),
    "PingFangSC-Semibold.otf": (12963768, "dc41610d27b99aa9f431ae642043320b67f44973"),
}


def blob_hash(data):
    return hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proxy", help="Optional proxy URL for curl; does not change system settings")
    parser.add_argument("--date", required=True, help="Download date in YYYY-MM-DD")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / "FontPackages" / "pingfang-sc"
    root.mkdir(parents=True, exist_ok=True)
    source = [
        f"Source: https://github.com/{REPO}",
        f"Pinned commit: {COMMIT}",
        f"Downloaded: {args.date}",
        "Original bytes preserved. Regular 400 / Medium 500 / Semibold 600.",
        "Font copyright: Copyright © 2015 DynaComware. All rights reserved.",
        "Upstream does not provide a separate redistribution license. See NOTICE.txt.",
        "Files verified against pinned GitHub Git blob SHA-1 values.",
        "",
        "Files and SHA-256:",
    ]
    for name, (size, expected) in FILES.items():
        target = root / name
        data = target.read_bytes() if target.exists() else b""
        url = f"https://raw.githubusercontent.com/{REPO}/{COMMIT}/{name}"
        if len(data) != size or blob_hash(data) != expected:
            step = 1024 * 1024

            def chunk(start):
                end = min(size - 1, start + step - 1)
                part = root / f".{name}.part-{start}.tmp"
                command = ["curl.exe", "--http1.1", "-fL", "--range", f"{start}-{end}",
                           "--connect-timeout", "15", "--max-time", "60", "--retry", "1",
                           "--silent", "--show-error", "-o", str(part), url]
                if args.proxy:
                    command.extend(["--proxy", args.proxy])
                try:
                    result = subprocess.run(command, capture_output=True, text=True,
                                            encoding="utf-8", errors="replace", check=False)
                    if result.returncode:
                        raise RuntimeError(f"{name} bytes {start}-{end}: {result.stderr.strip()}")
                    block = part.read_bytes()
                    if len(block) != end - start + 1:
                        raise RuntimeError(f"Unexpected range response for {name} at {start}")
                    print(f"{name}: received bytes {start}-{end}", flush=True)
                    return block
                finally:
                    part.unlink(missing_ok=True)

            with ThreadPoolExecutor(max_workers=6) as pool:
                data = b"".join(pool.map(chunk, range(0, size, step)))
            if len(data) != size or blob_hash(data) != expected:
                raise RuntimeError(f"Font does not match pinned GitHub blob: {name}")
            staged = root / f".{name}.verified.tmp"
            staged.write_bytes(data)
            staged.replace(target)
        print(f"{name}: verified {len(data)} bytes", flush=True)
        source.extend([f"{hashlib.sha256(data).hexdigest()}  {name}",
                       f"Git blob SHA-1: {expected}", url])
    (root / "SOURCE.txt").write_text("\n".join(source) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
