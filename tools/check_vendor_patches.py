#!/usr/bin/env python3
"""Offline vendor identity and complete content comparison; no downloads."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parent.parent
IDENTITIES = {
    "egui_tiles": ("0.16.0", "9eb8fef6130bd04fcb7bb3584845605e57c56fed249bc3ca5a568e696cc0a174", {
        "src/behavior.rs", "src/container/linear.rs", "src/container/tabs.rs", "src/lib.rs", "src/tree.rs"}),
    "cosmic-text": ("0.19.0", "be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73", {
        "Cargo.toml", "Cargo.toml.orig", "src/attrs.rs", "src/font/mod.rs", "src/font/system.rs", "src/shape.rs", "src/shape_run_cache.rs"}),
}

def files(root, exclude):
    return {p.relative_to(root).as_posix(): p.read_bytes().replace(b"\r\n", b"\n")
            for p in root.rglob("*") if p.is_file() and p.relative_to(root).as_posix() not in exclude}

class ArchiveNotCached(Exception):
    """Verification unavailable; distinct from a content mismatch."""


def check(name):
    version, expected_hash, expected_modified = IDENTITIES[name]
    cargo = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    archives = list((cargo / "registry/cache").glob(f"*/{name}-{version}.crate"))
    if not archives:
        raise ArchiveNotCached(f"{name}-{version}.crate")
    archive = archives[0]
    if hashlib.sha256(archive.read_bytes()).hexdigest() != expected_hash:
        raise ValueError(f"{name}: archive hash mismatch")
    exclude = {".cargo-ok", ".cargo_vcs_info.json"}
    if name == "egui_tiles": exclude.add("Cargo.toml.orig")
    else: exclude.update({"Cargo.lock", ".gitattributes"})  # ignored upstream library lock; workspace lock is authoritative
    with tempfile.TemporaryDirectory(prefix="varos-vendor-") as temp:
        with tarfile.open(archive) as tar:
            for member in tar.getmembers():
                if member.issym() or member.islnk() or not (Path(temp) / member.name).resolve().is_relative_to(Path(temp).resolve()):
                    raise ValueError("unsafe archive member")
            tar.extractall(temp)
        upstream = Path(temp) / f"{name}-{version}"
        pristine = files(upstream, exclude)
        vendor = files(ROOT / "varos/vendor" / name, exclude)
        if pristine.keys() != vendor.keys(): raise ValueError(f"{name}: file set changed")
        changed = {p for p in pristine if pristine[p] != vendor[p]}
        if changed != expected_modified: raise ValueError(f"{name}: unexpected delta: {sorted(changed)}")
        if name == "cosmic-text":
            ledger = ROOT / "varos/vendor/patches"
            for filename, digest in {
                "cosmic-text-0.19.0-tatweel.patch": "8d9320ed7cdcf55114decab9f9bcc6b3d41aeb0de3b60c62207fdc587412e6ad",
                "pristine.sha256": "f7787ee3330565390213ce1f4e76d6425daae3876c24bd0df375fa8f575bb548",
                "cosmic-text-0.19.0-p1b.patch": "a2261107836d022b7bd7dddbb45d059e9b14038653705b76fc2c53d4f131f654",
            }.items():
                if hashlib.sha256((ledger / filename).read_bytes()).hexdigest() != digest:
                    raise ValueError(f"{name}: ledger identity mismatch: {filename}")
            for line in (ledger / "pristine.sha256").read_text().splitlines():
                digest, relative = line.split("  ", 1)
                if relative == ".cargo-ok": continue  # registry-only marker, absent from archive
                if hashlib.sha256((upstream / relative).read_bytes()).hexdigest() != digest:
                    raise ValueError(f"{name}: pristine hash mismatch: {relative}")
            subprocess.run(["patch", "-p1", "--silent"], cwd=upstream,
                           input=(ledger / "cosmic-text-0.19.0-p1b.patch").read_bytes(), check=True)
            subprocess.run(["patch", "-p1", "--silent"], cwd=upstream,
                           input=(ledger / "cosmic-text-0.19.0-tatweel.patch").read_bytes(), check=True)
            if files(upstream, exclude) != vendor: raise ValueError("cosmic-text: patch does not reproduce vendor")
        print(f"{name}: PASS; archive SHA-256 {expected_hash}; {len(pristine)} files; {len(changed)} modified")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", choices=IDENTITIES)
    args = parser.parse_args()
    failed = 0
    skipped = 0
    for name in [args.package] if args.package else IDENTITIES:
        try: check(name)
        except ArchiveNotCached:
            print(f"{name}: SKIP (archive not cached; not verified)")
            skipped += 1
        except (ValueError, OSError, subprocess.CalledProcessError) as error:
            print(f"{name}: FAIL - {error}")
            failed += 1
    status = "FAIL" if failed else "SKIP (archive not cached; not verified)" if skipped else "PASS"
    print(f"check_vendor_patches: {status}")
    return 1 if failed else 2 if skipped else 0

if __name__ == "__main__":
    raise SystemExit(main())
