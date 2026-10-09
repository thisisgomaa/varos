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
    ]
}
