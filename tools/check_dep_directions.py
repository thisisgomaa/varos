#!/usr/bin/env python3
"""Enforce ADR-0005 using Python's standard library, from any working directory."""

import json
from pathlib import Path
import re
import subprocess
import sys

EDGES = {
    "varos-core": set(),
    "varos-render-wgpu": {"varos-core"},
    "varos-pdf": {"varos-core"},
    "varos-app": {"varos-core", "varos-render-wgpu", "varos-pdf", "varos-raster", "varos-bridge"},
    "varos-raster": {"varos-core", "varos-pdf"},
    "varos-bridge": {"varos-core"},
    "varos-cli": {"varos-core", "varos-pdf", "varos-raster", "varos-bridge"},
}


def validate(metadata, app_source):
    """Return violations, including target-specific, build and dev dependencies."""
    violations = []

    def exact(label, actual, expected):
        if set(actual) != set(expected):
            violations.append(f"{label}: found {sorted(actual)}, expected {sorted(expected)}")

    member_ids = set(metadata["workspace_members"])
    packages = {p["name"]: p for p in metadata["packages"] if p["id"] in member_ids}
    exact("workspace members", packages, EDGES)
    for name, allowed in EDGES.items():
        if name not in packages:
            continue  # Already reported by the workspace assertion.
        dependencies = packages[name]["dependencies"]
        internal = {dep["name"] for dep in dependencies if dep["name"] in packages}
        exact(f"{name} internal dependencies", internal, allowed)
        for dependency in dependencies:
            normalized = dependency["name"].replace("_", "-")
            if name in {"varos-core", "varos-raster", "varos-cli", "varos-bridge"} and re.match(
                r"^(wgpu|winit|egui(?:-|$)|windows(?:-|$))", normalized
            ):
                violations.append(f"{name} forbidden UI/GPU/platform dependency: {dependency['name']}")
            if name == "varos-render-wgpu" and re.match(r"^winit(?:-|$)", normalized):
                violations.append("varos-render-wgpu must not depend on winit")
        if name == "varos-app" and sum(d["name"] == "egui_tiles" for d in dependencies) != 1:
            violations.append("varos-app must declare exactly one egui_tiles dependency")

    # Preserve the original PowerShell gate's comment-stripped identifier scan.
    # This is an architectural tripwire, not a Rust parser.
    users = set()
    for source in app_source.rglob("*.rs"):
        code = re.sub(r"/\*.*?\*/", "", source.read_text(encoding="utf-8"), flags=re.S)
        code = re.sub(r"//[^\n]*", "", code)
        if re.search(r"\begui_tiles\b", code):
            users.add(source.relative_to(app_source).as_posix())
    exact("varos-app egui_tiles code use", users, {"shell/boxtree.rs"})
    return violations


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps",
             "--manifest-path", str(root / "varos/Cargo.toml")],
            check=True, capture_output=True, text=True, encoding="utf-8",
        )
        violations = validate(json.loads(result.stdout), root / "varos/crates/varos-app/src")
        if violations:
            for violation in violations:
                print(f"ERROR: {violation}", file=sys.stderr)
            print(f"check_dep_directions: FAIL ({len(violations)} violation(s))", file=sys.stderr)
            return 1
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"check_dep_directions: FAIL - {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        return 1
    print("check_dep_directions: PASS")
    print("internal edges: Bridge -> core; raster -> core (+ pdf tests); CLI -> Bridge, core, pdf, raster; app -> core, renderer, pdf, raster, Bridge")
    print("egui_tiles code use: varos-app/src/shell/boxtree.rs only")
    return 0


if __name__ == "__main__":
    sys.exit(main())
