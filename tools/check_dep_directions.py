#!/usr/bin/env python3
"""Enforce ADR-0005 using Python's standard library, from any working directory."""

import json
from pathlib import Path
import re
import subprocess
import sys

EDGES = {
    "varos-text-layout": {"varos-core", "varos-text"},
    "varos-core": set(),
    "varos-import": {"varos-core"},
    "varos-text": set(),
    "varos-render-wgpu": {"varos-core"},
    "varos-pdf": {"varos-text-layout", "varos-core"},
    "varos-app": {"varos-text-layout", "varos-import", "varos-text", "varos-core", "varos-render-wgpu", "varos-pdf", "varos-raster", "varos-bridge"},
    "varos-raster": {"varos-text-layout", "varos-core", "varos-pdf"},  # PDF is test-only.
    "varos-bridge": {"varos-core", "varos-raster"},
    "varos-cli": {"varos-text-layout", "varos-import", "varos-core", "varos-pdf", "varos-raster", "varos-bridge"},
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
            if name in {"varos-import", "varos-text", "varos-text-layout", "varos-core", "varos-raster", "varos-cli", "varos-bridge"} and re.match(
                r"^(wgpu|winit|epaint(?:-|$)|egui(?:-|$)|windows(?:-|$))", normalized
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
            ["cargo", "metadata", "--offline", "--locked", "--format-version", "1", "--no-deps",
             "--manifest-path", str(root / "varos/Cargo.toml")],
            check=True, capture_output=True, text=True, encoding="utf-8",
        )
        violations = validate(json.loads(result.stdout), root / "varos/crates/varos-app/src")
        graph_unresolved = False
        try:
            graph = subprocess.run(
                ["cargo", "tree", "--offline", "--locked", "-e", "features", "-p", "varos-text",
                 "--manifest-path", str(root / "varos/Cargo.toml")],
                check=True, capture_output=True, text=True,
            ).stdout
        except (OSError, subprocess.CalledProcessError) as error:
            graph = ""
            graph_unresolved = True
            print("check_dep_directions: SKIP isolated feature graph (cargo tree cannot resolve; not verified)", file=sys.stderr)
            print(getattr(error, "stderr", None) or str(error), file=sys.stderr)
        forbidden = r'\b(?:sys-locale|swash|egui(?:-[\w]+)?|epaint|winit|wgpu(?:-[\w]+)?) v|cosmic-text feature "(?:system-discovery|fontconfig)"|fontdb feature "(?:fontconfig|fs|memmap)"'
        if re.search(forbidden, graph):
            violations.append("varos-text isolated graph contains discovery/UI/GPU features")
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
    if graph_unresolved:
        print("dependency edges checked; isolated features not verified", file=sys.stderr)
        return 2
    print("check_dep_directions: PASS")
    print("internal edges: import -> core; app and CLI -> import; core -> import forbidden; Bridge -> core, raster; raster -> core (+ pdf tests); CLI -> Bridge, core, pdf, raster; app -> core, renderer, pdf, raster, Bridge")
    print("varos-text: pure leaf; app -> text allowed; core -> text forbidden until T4; isolated features clean")
    print("egui_tiles code use: varos-app/src/shell/boxtree.rs only")
    return 0


if __name__ == "__main__":
    sys.exit(main())
