use super::*;

/// A shortcut hint shown in an icon button's tooltip.
#[derive(Clone, Copy)]
pub(super) enum Hint {
    None,
    /// The platform primary modifier + key (⌘G on Mac, Ctrl+G elsewhere).
    Primary(&'static str),
    AltPrimary(&'static str),
    /// A plain key name.
    Key(&'static str),
}

/// One panel icon button: its stable key, its registry glyph, the text label it used to show (now its
/// tooltip) and its shortcut. Every icon button in the panels is drawn from [`ICON_ACTIONS`], so the
/// tooltip test covers all of them (ICON_LIBRARY_STUDY §4; owner: "icons instead of text").
#[derive(Clone, Copy)]
pub(super) struct IconAction {
    pub(super) key: &'static str,
    pub(super) icon: Icon,
    pub(super) label: &'static str,
    hint: Hint,
}
impl IconAction {
    /// The label plus its shortcut — what the button says on hover.
    pub(super) fn tooltip(&self) -> String {
        match self.hint {
            Hint::None => self.label.to_string(),
            Hint::Primary(k) => format!("{} ({})", self.label, shortcut_label(k)),
            Hint::AltPrimary(k) => format!(
                "{} ({}{})",
                self.label,
                if cfg!(target_os = "macos") { "⌥" } else { "Alt+" },
                shortcut_label(k)
            ),
            Hint::Key(k) => format!("{} ({k})", self.label),
        }
    }
    /// Draw it through the one kit control; true once on activation (pointer or Enter/Space).
    pub(super) fn show(&self, ui: &mut egui::Ui, state: kit::IconState<'_>) -> bool {
        self.show_response(ui, state).activated
    }
    pub(super) fn show_sized(
        &self,
        ui: &mut egui::Ui,
        state: kit::IconState<'_>,
        size: egui::Vec2,
        glyph: f32,
    ) -> bool {
        let id = ui.make_persistent_id(("icon-action", self.key));
        let r = kit::icon_button_sized(ui, id, self.icon, &self.tooltip(), state, size, glyph);
        #[cfg(test)]
        tests::icon_action_tests::PROBE.with(|p| p.borrow_mut().push((self.key, id, r.response.rect)));
        r.activated
    }
    pub(super) fn show_response(&self, ui: &mut egui::Ui, state: kit::IconState<'_>) -> kit::ControlResponse {
        let id = ui.make_persistent_id(("icon-action", self.key));
        let r = kit::icon_button(ui, id, self.icon, &self.tooltip(), state);
        #[cfg(test)]
        tests::icon_action_tests::PROBE.with(|p| p.borrow_mut().push((self.key, id, r.response.rect)));
        r
    }
}

pub(super) const IA_LAYER_GROUP: IconAction =
    IconAction { key: "layer-group", icon: Icon::Group, label: "Group the selection", hint: Hint::Primary("G") };
pub(super) const IA_LAYER_DELETE: IconAction =
    IconAction { key: "layer-delete", icon: Icon::Trash, label: "Delete the selection", hint: Hint::Key("Delete") };
pub(super) const IA_AB_ADD: IconAction =
    IconAction { key: "ab-add", icon: Icon::ArtboardAdd, label: "Add artboard", hint: Hint::None };
pub(super) const IA_AB_DUP: IconAction =
    IconAction { key: "ab-dup", icon: Icon::Duplicate, label: "Duplicate artboard", hint: Hint::None };
pub(super) const IA_AB_DEL: IconAction =
    IconAction { key: "ab-del", icon: Icon::Trash, label: "Delete artboard", hint: Hint::None };
pub(super) const IA_AB_LINK: IconAction =
    IconAction { key: "ab-link", icon: Icon::Link, label: "Constrain W/H", hint: Hint::None };
pub(super) const IA_AB_PORTRAIT: IconAction =
    IconAction { key: "ab-portrait", icon: Icon::Portrait, label: "Portrait", hint: Hint::None };
pub(super) const IA_AB_LANDSCAPE: IconAction =
    IconAction { key: "ab-landscape", icon: Icon::Landscape, label: "Landscape", hint: Hint::None };
pub(super) const IA_AB_FIT: IconAction =
    IconAction { key: "ab-fit", icon: Icon::Fit, label: "Fit in window", hint: Hint::Primary("0") };
pub(super) const IA_PROP_LINK: IconAction =
    IconAction { key: "prop-link", icon: Icon::Link, label: "Constrain W/H proportions", hint: Hint::None };
pub(super) const IA_FLIP_H: IconAction =
    IconAction { key: "flip-h", icon: Icon::FlipH, label: "Flip horizontal", hint: Hint::None };
pub(super) const IA_FLIP_V: IconAction =
    IconAction { key: "flip-v", icon: Icon::FlipV, label: "Flip vertical", hint: Hint::None };
pub(super) const IA_NO_FILL: IconAction =
    IconAction { key: "no-fill", icon: Icon::Remove, label: "No paint", hint: Hint::None };
pub(super) const IA_NO_STROKE: IconAction =
    IconAction { key: "no-stroke", icon: Icon::Remove, label: "No paint", hint: Hint::None };
pub(super) const IA_PICKER_CLOSE: IconAction =
    IconAction { key: "picker-close", icon: Icon::Remove, label: "Close", hint: Hint::Key("Esc") };

/// Every panel icon action, for the tooltip/emission tests.
#[cfg(test)]
pub(super) const ICON_ACTIONS: [IconAction; 40] = [
    IA_LAYER_NEW,
    IA_AB_EARLIER,
    IA_AB_LATER,
    IA_AB_FIT_ART,
    IA_AB_FIT_SELECTION,
    IA_AB_CONVERT,
    IA_LAYER_GROUP,
    IA_LAYER_FILTER,
    IA_LAYER_DELETE,
    IA_AB_ADD,
    IA_AB_DUP,
    IA_AB_DEL,
    IA_AB_LINK,
    IA_AB_PORTRAIT,
    IA_AB_LANDSCAPE,
    IA_AB_FIT,
    IA_PROP_LINK,
    IA_FLIP_H,
    IA_FLIP_V,
    IA_NO_FILL,
    IA_NO_STROKE,
    IA_PICKER_CLOSE,
    IA_STROKE_PRESETS,
    IA_CLIP,
    IA_MOVE,
    IA_TRANSPARENT,
    IA_OBJECT_CLIP,
    IA_SNAP,
    IA_GUIDES,
    IA_RULERS,
    IA_GRID,
    IA_GUIDES_LOCK,
    IA_SMART,
    IA_POINT,
    IA_SNAP_GRID,
    IA_EDIT_BOARDS,
    IA_FIT_BOARD,
    IA_ADD_BOARD,
    IA_PREV_BOARD,
    IA_NEXT_BOARD,
];

pub(super) const IA_CLIP: IconAction =
    IconAction { key: "clip", icon: Icon::Crop, label: "Clip to page", hint: Hint::None };
pub(super) const IA_MOVE: IconAction =
    IconAction { key: "move", icon: Icon::MoveArtwork, label: "Move artwork with artboard", hint: Hint::None };
pub(super) const IA_TRANSPARENT: IconAction =
    IconAction { key: "transparent", icon: Icon::TransparentPage, label: "Transparent page", hint: Hint::None };
pub(super) const IA_OBJECT_CLIP: IconAction = IconAction {
    key: "object_clip",
    icon: Icon::Crop,
    label: "Clip to artboard — off lets this object bleed",
    hint: Hint::None,
};
pub(super) const IA_SNAP: IconAction =
    IconAction { key: "snap", icon: Icon::Magnet, label: "Snapping", hint: Hint::None };
pub(super) const IA_GUIDES: IconAction =
    IconAction { key: "guides", icon: Icon::Guides, label: "Guides", hint: Hint::Primary(";") };
pub(super) const IA_RULERS: IconAction =
    IconAction { key: "rulers", icon: Icon::Ruler, label: "Rulers", hint: Hint::Primary("R") };
pub(super) const IA_GRID: IconAction = IconAction {
    key: "grid",
    icon: Icon::GridDots,
    label: "Grid dots — always shown (no switch yet)",
    hint: Hint::None,
};
pub(super) const IA_GUIDES_LOCK: IconAction =
    IconAction { key: "guides_lock", icon: Icon::GuidesLock, label: "Lock guides", hint: Hint::AltPrimary(";") };
pub(super) const IA_SMART: IconAction =
    IconAction { key: "smart", icon: Icon::SmartGuides, label: "Smart guides", hint: Hint::Primary("U") };
pub(super) const IA_POINT: IconAction =
    IconAction { key: "point", icon: Icon::SnapPoint, label: "Snap to point", hint: Hint::None };
pub(super) const IA_SNAP_GRID: IconAction =
    IconAction { key: "snap_grid", icon: Icon::SnapGrid, label: "Snap to grid", hint: Hint::None };
pub(super) const IA_EDIT_BOARDS: IconAction =
    IconAction { key: "edit_boards", icon: Icon::Frame, label: "Edit artboards", hint: Hint::Key("Shift+O") };
pub(super) const IA_FIT_BOARD: IconAction =
    IconAction { key: "fit_board", icon: Icon::Fit, label: "Fit artboard in window", hint: Hint::Primary("0") };
pub(super) const IA_ADD_BOARD: IconAction =
    IconAction { key: "add_board", icon: Icon::ArtboardAdd, label: "Add artboard", hint: Hint::None };
pub(super) const IA_PREV_BOARD: IconAction =
    IconAction { key: "prev_board", icon: Icon::ChevronLeft, label: "Previous artboard", hint: Hint::None };
pub(super) const IA_NEXT_BOARD: IconAction =
    IconAction { key: "next_board", icon: Icon::ChevronRight, label: "Next artboard", hint: Hint::None };

pub(super) const IA_STROKE_PRESETS: IconAction =
    IconAction { key: "stroke-presets", icon: Icon::ChevronDown, label: "Stroke weight presets", hint: Hint::None };

pub(super) const IA_LAYER_FILTER: IconAction =
    IconAction { key: "layer-filter", icon: Icon::ListFilter, label: "Filter layers by kind", hint: Hint::None };

pub(super) const IA_LAYER_NEW: IconAction =
    IconAction { key: "layer-new", icon: Icon::New, label: "New Layer (Option: New Sublayer)", hint: Hint::None };

pub(super) const IA_AB_EARLIER: IconAction =
    IconAction { key: "ab-earlier", icon: Icon::ChevronLeft, label: "Move artboard earlier", hint: Hint::None };
pub(super) const IA_AB_LATER: IconAction =
    IconAction { key: "ab-later", icon: Icon::ChevronRight, label: "Move artboard later", hint: Hint::None };
pub(super) const IA_AB_FIT_ART: IconAction =
    IconAction { key: "ab-fit-art", icon: Icon::Fit, label: "Fit artboard to artwork bounds", hint: Hint::None };
pub(super) const IA_AB_FIT_SELECTION: IconAction = IconAction {
    key: "ab-fit-selection",
    icon: Icon::AlignSelection,
    label: "Fit artboard to selected art",
    hint: Hint::None,
};
pub(super) const IA_AB_CONVERT: IconAction = IconAction {
    key: "ab-convert",
    icon: Icon::ArtboardAdd,
    label: "Convert selected art to artboards",
    hint: Hint::None,
};
