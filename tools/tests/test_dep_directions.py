"""Negative checks: architectural violations must fail the gate."""

from contextlib import redirect_stderr, redirect_stdout
import io
import json
import subprocess
from unittest.mock import patch
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_dep_directions import main, validate


class DependencyDirections(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.source = Path(self.temp.name)
        (self.source / "shell").mkdir()
        (self.source / "shell/boxtree.rs").write_text("use egui_tiles::Tree;", encoding="utf-8")
        self.packages = [
            {"id": name, "name": name, "dependencies": [{"name": d} for d in deps]}
            for name, deps in [
                ("varos-core", ["serde"]),
                ("varos-render-wgpu", ["varos-core", "wgpu"]),
                ("varos-pdf", ["varos-core"]),
                ("varos-app", ["varos-core", "varos-render-wgpu", "varos-pdf", "varos-raster", "varos-bridge", "varos-text", "varos-import", "egui_tiles"]),
                ("varos-raster", ["varos-core", "varos-pdf", "tiny-skia"]),
                ("varos-cli", ["varos-core", "varos-pdf", "varos-raster", "varos-bridge", "varos-import"]),
                ("varos-text", ["cosmic-text", "skrifa", "fontdb"]),
                ("varos-bridge", ["varos-core", "varos-raster"]),
                ("varos-import", ["varos-core", "usvg"]),
            ]
        ]
        self.metadata = {"packages": self.packages, "workspace_members": [p["id"] for p in self.packages]}

    def check(self):
        return validate(self.metadata, self.source)

    def test_allowed_graph_and_comment_mentions_pass(self):
        (self.source / "main.rs").write_text(
            "// egui_tiles lives elsewhere\n/* egui_tiles */ fn main() {}", encoding="utf-8"
        )
        self.assertEqual(self.check(), [])

    def test_reverse_edge_fails_even_when_renamed(self):
        self.packages[0]["dependencies"].append({"name": "varos-app", "rename": "shell"})
        self.assertTrue(any("varos-core internal dependencies" in e for e in self.check()))

    def test_target_specific_platform_dependency_fails(self):
        self.packages[0]["dependencies"].append({"name": "windows-sys", "target": "cfg(windows)"})
        self.assertTrue(any("forbidden" in e for e in self.check()))

    def test_headless_crates_reject_ui_even_in_dev_or_target_dependencies(self):
        for name in ("varos-raster", "varos-cli", "varos-text", "varos-import"):
            for dependency in ("wgpu", "winit", "egui", "egui-wgpu", "windows-sys"):
                with self.subTest(crate=name, dependency=dependency):
                    package = next(p for p in self.packages if p["name"] == name)
                    package["dependencies"].append({
                        "name": dependency, "kind": "dev", "target": "cfg(windows)",
                    })
                    self.assertTrue(any(f"{name} forbidden" in e for e in self.check()))
                    package["dependencies"].pop()

    def test_text_remains_a_leaf_and_core_edge_waits_for_t4(self):
        self.packages[0]["dependencies"].append({"name": "varos-text"})
        self.assertTrue(any("varos-core internal dependencies" in e for e in self.check()))
        self.packages[0]["dependencies"].pop()
        text = next(p for p in self.packages if p["name"] == "varos-text")
        text["dependencies"].append({"name": "varos-core"})
        self.assertTrue(any("varos-text internal dependencies" in e for e in self.check()))

    def test_core_cannot_depend_on_foreign_import(self):
        self.packages[0]["dependencies"].append({"name": "varos-import"})
        self.assertTrue(any("varos-core internal dependencies" in e for e in self.check()))

    def test_renderer_window_dependency_fails(self):
        self.packages[1]["dependencies"].append({"name": "winit"})
        self.assertTrue(any("must not depend on winit" in e for e in self.check()))

    def test_missing_member_fails(self):
        self.metadata["workspace_members"].remove("varos-pdf")
        self.assertTrue(any("workspace members" in e for e in self.check()))

    def test_tiles_use_outside_adapter_fails(self):
        (self.source / "main.rs").write_text("use egui_tiles::Tree;", encoding="utf-8")
        self.assertTrue(any("main.rs" in e for e in self.check()))

    def test_missing_tiles_dependency_fails(self):
        self.packages[3]["dependencies"].pop()
        self.assertTrue(any("exactly one egui_tiles" in e for e in self.check()))

    def test_tree_and_metadata_stay_offline_locked(self):
        with patch("check_dep_directions.subprocess.run", side_effect=[
            subprocess.CompletedProcess([], 0, stdout=json.dumps(self.metadata)),
            subprocess.CompletedProcess([], 0, stdout="cosmic-text feature std"),
        ]) as run, patch("check_dep_directions.validate", return_value=[]), redirect_stdout(io.StringIO()):
            self.assertEqual(main(), 0)
        metadata_args, tree_args = [call.args[0] for call in run.call_args_list]
        self.assertIn("--offline", metadata_args)
        self.assertIn("--no-deps", metadata_args)
        self.assertIn("--locked", tree_args)
        self.assertIn("--offline", tree_args)

    def test_unresolved_tree_skips_but_known_violation_still_fails(self):
        for violations, expected in [([], 2), (["bad edge"], 1)]:
            stderr = io.StringIO()
            with patch("check_dep_directions.subprocess.run", side_effect=[
                subprocess.CompletedProcess([], 0, stdout=json.dumps(self.metadata)),
                subprocess.CalledProcessError(101, ["cargo", "tree"], stderr="cannot resolve"),
            ]), patch("check_dep_directions.validate", return_value=violations), redirect_stderr(stderr):
                self.assertEqual(main(), expected)
            self.assertIn("not verified", stderr.getvalue())

    def test_forbidden_feature_graph_fails(self):
        with patch("check_dep_directions.subprocess.run", side_effect=[
            subprocess.CompletedProcess([], 0, stdout=json.dumps(self.metadata)),
            subprocess.CompletedProcess([], 0, stdout='fontdb feature "fs"'),
        ]), patch("check_dep_directions.validate", return_value=[]), redirect_stderr(io.StringIO()):
            self.assertEqual(main(), 1)


if __name__ == "__main__":
    unittest.main()
