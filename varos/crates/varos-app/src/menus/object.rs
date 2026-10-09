//! The Object menu: Transform Again, Arrange, Group / Ungroup.

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        key("obj.again", "Transform Again", cmd(K::KeyD)),
        Entry::Sep,
        Entry::Sub {
            label: "Arrange",
            items: vec![
                key("obj.front", "Bring to Front", cmd_shift(K::BracketRight)),
                key("obj.forward", "Bring Forward", cmd(K::BracketRight)),
                key("obj.backward", "Send Backward", cmd(K::BracketLeft)),
                key("obj.back", "Send to Back", cmd_shift(K::BracketLeft)),
            ],
        },
        Entry::Sub {
            label: "Transform",
            items: ["Rotate…", "Scale…", "Reflect…", "Shear…", "Transform Each…"]
                .into_iter()
                .map(|label| Entry::Item {
                    id: format!("4a.{label}"),
                    label,
                    accel: None,
                    cmd: MenuCmd::Slice4a(label),
                    check: None,
                })
                .collect(),
        },
        Entry::Sep,
        key("obj.group", "Group", cmd(K::KeyG)),
        key("obj.ungroup", "Ungroup", cmd_shift(K::KeyG)),
        Entry::Sub {
            label: "Clipping Mask",
            items: vec![
                key("obj.clip", "Make", cmd(K::Digit7)),
                key("obj.release_clip", "Release", cmd_alt(K::Digit7)),
            ],
        },
        Entry::Sep,
        Entry::Sub {
            label: "Layers",
            items: vec![
                Entry::Item {
                    id: "obj.pastelayers".into(),
                    label: "Paste Remembers Layers",
                    accel: None,
                    cmd: MenuCmd::TogglePasteRemembersLayers,
                    check: Some(Check::PasteRemembersLayers),
                },
                item(
                    "obj.newlayer",
                    "New Layer",
                    None,
                    MenuCmd::Object(varos_core::editor::wave::ObjectAction::NewLayer),
                ),
                item(
                    "obj.newsublayer",
                    "New Sublayer",
                    None,
                    MenuCmd::Object(varos_core::editor::wave::ObjectAction::NewSublayer),
                ),
                item(
                    "obj.currentlayer",
                    "Send to Current Layer",
                    None,
                    MenuCmd::Object(varos_core::editor::wave::ObjectAction::SendToCurrentLayer),
                ),
            ],
        },
        Entry::Sub { label: "Lock", items: vec![key("obj.lock", "Selection", cmd(K::Digit2))] },
        key("obj.unlock", "Unlock All", cmd_alt(K::Digit2)),
        Entry::Sub { label: "Hide", items: vec![key("obj.hide", "Selection", cmd(K::Digit3))] },
        key("obj.show", "Show All", cmd_alt(K::Digit3)),
        item(
            "obj.expandtransform",
            "Expand Transform",
            None,
            MenuCmd::Object(varos_core::editor::wave::ObjectAction::ExpandTransform),
        ),
        // ---- Lane C ----
        item("obj.expand", "Expand", None, MenuCmd::LaneC("Expand")),
        Entry::Sub {
            label: "Path",
            items: vec![
                item("obj.outline-stroke", "Outline Stroke", None, MenuCmd::LaneC("Outline Stroke")),
                item("obj.offset-path", "Offset Path…", None, MenuCmd::LaneC("Offset Path…")),
                key("obj.join", "Join", cmd(K::KeyJ)),
                key("obj.average", "Average", cmd_alt(K::KeyJ)),
                item(
                    "obj.addanchors",
                    "Add Anchor Points",
                    None,
                    MenuCmd::Object(varos_core::editor::wave::ObjectAction::AddAnchors),
                ),
                item(
                    "obj.reverse",
                    "Reverse Path Direction",
                    None,
                    MenuCmd::Object(varos_core::editor::wave::ObjectAction::Reverse),
                ),
                item("obj.cleanup", "Clean Up", None, MenuCmd::Object(varos_core::editor::wave::ObjectAction::CleanUp)),
            ],
        },
        Entry::Sub {
            label: "Compound Path",
            items: vec![
                key("obj.compound.make", "Make", cmd(K::Digit8)),
                key(
                    "obj.compound.release",
                    "Release",
                    Some(Accel { code: K::Digit8, cmd: true, shift: true, alt: true }),
                ),
            ],
        },
    ]
}
