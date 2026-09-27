"""Negative checks: architectural violations must fail the gate."""

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_dep_directions import validate


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
                ("varos-app", ["varos-core", "varos-render-wgpu", "varos-pdf", "egui_tiles"]),
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


if __name__ == "__main__":
    unittest.main()
