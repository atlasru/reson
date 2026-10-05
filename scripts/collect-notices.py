"""Collect upstream license notices from the exact locked build dependencies."""
import hashlib
import json
import os
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "src-tauri/resources/licenses/DEPENDENCIES.txt"
notices = {}
missing = []
fallbacks = {
    "alloc-stdlib": "alloc-stdlib-BSD.txt",
    "defmt-parser": "defmt-MIT.txt",
    "selectors": "MPL-2.0.txt",
    "webview2-com": "webview2-MIT.txt",
    "webview2-com-macros": "webview2-MIT.txt",
    "webview2-com-sys": "webview2-MIT.txt",
}


def collect(name, root, declared):
    candidates = []
    for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "Copyright*", "license*", "licence*"):
        for path in root.glob(pattern):
            candidates.extend(path.rglob("*") if path.is_dir() else [path])
    found = False
    for path in sorted(set(candidates)):
        if not path.is_file() or path.stat().st_size > 512_000:
            continue
        try:
            text = path.read_text(encoding="utf-8").strip()
        except UnicodeDecodeError:
            continue
        if not text:
            continue
        found = True
        key = hashlib.sha256(text.encode()).hexdigest()
        group = notices.setdefault(key, {"text": text, "packages": set()})
        group["packages"].add(f"{name} ({declared})")
    if not found:
        parts = name.split()
        fallback = fallbacks.get(parts[1]) if len(parts)>1 else None
        if fallback and (OUTPUT.parent / fallback).is_file():
            text = (OUTPUT.parent / fallback).read_text(encoding="utf-8").strip()
            key = hashlib.sha256(text.encode()).hexdigest()
            group = notices.setdefault(key, {"text":text,"packages":set()})
            group["packages"].add(f"{name} ({declared}; upstream workspace license)")
        else:
            missing.append(f"{name} ({declared})")


metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--format-version", "1", "--locked",
    "--filter-platform", "x86_64-pc-windows-msvc",
], cwd=ROOT))
used = {node["id"] for node in metadata["resolve"]["nodes"]}
for package in metadata["packages"]:
    if package["id"] not in used or not package.get("source"):
        continue
    collect("Rust: " + package["name"] + " " + package["version"],
            pathlib.Path(package["manifest_path"]).parent, package.get("license") or "upstream")

tree = json.loads(subprocess.check_output(["npm", "ls", "--omit=dev", "--json", "--all"],
                                         cwd=ROOT, shell=(os.name == "nt")))


def walk(dependencies, parent=ROOT):
    for name, value in dependencies.items():
        directory = parent / "node_modules" / name
        if not directory.exists():
            directory = ROOT / "node_modules" / name
        manifest = directory / "package.json"
        if manifest.exists():
            package = json.loads(manifest.read_text())
            collect("npm: " + name + " " + package["version"], directory,
                    str(package.get("license", "upstream")))
        walk(value.get("dependencies", {}), directory)


walk(tree.get("dependencies", {}))
lines = ["Reson bundled dependency license notices", "",
         "Generated from Cargo.lock and package-lock.json. Upstream texts follow.",
         "Build/test dependency notices may also be included.", ""]
for group in sorted(notices.values(), key=lambda group: sorted(group["packages"])[0]):
    lines.extend(["=" * 72, *sorted(group["packages"]), "", group["text"], ""])
if missing:
    lines.extend(["Packages with no root license file (declared SPDX identifiers):",
                  *sorted(set(missing)), ""])
OUTPUT.parent.mkdir(parents=True, exist_ok=True)
OUTPUT.write_text("\n".join(lines), encoding="utf-8")
print(f"Collected {len(notices)} unique upstream notices; {len(set(missing))} SPDX-only entries")
