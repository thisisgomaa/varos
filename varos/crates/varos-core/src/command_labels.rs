//! Lane F: semantic human history labels, separate from localized surface placements.
use crate::EditCommand;
pub(crate) fn label(command: &EditCommand) -> &'static str {
    use EditCommand::*;
    match command {
        // ---- Lane B w3-effects ----
        LiveEffects(crate::effects::Action::Set { .. } | crate::effects::Action::SetPerPath { .. }) => {
            "Set live effects"
        }
        LiveEffects(crate::effects::Action::Expand { .. }) => "Expand live effects",
        LiveEffects(crate::effects::Action::Width { .. } | crate::effects::Action::WidthLive { .. }) => {
            "Change width profile"
        }
        // ---- end Lane B w3-effects ----
        Nudge { .. } => "Move",
        SetOpacity(_) => "Change opacity",
        AddShape { .. } => "Draw shape",
        AddPath { .. } => "Draw path",
        DeleteSelected | DeleteAnchor(_) => "Delete",
        ApplyPaint { .. } | SwapColors | DefaultPaint => "Change paint",
        SetStrokeWidth(_) | SetStrokeStyle { .. } => "Change stroke",
        SetObjectBounds { .. } => "Transform bounds",
        SetObjectRotation(_) | Transform(_) | TransformCommit => "Transform",
        GroupSelection => "Group",
        UngroupSelection => "Ungroup",
        Arrange(_) => "Arrange",
        ClipMake => "Make clipping mask",
        ClipRelease => "Release clipping mask",
        Copy => "Copy",
        Cut => "Cut",
        Paste { .. } => "Paste",
        SetBoardName(_) | SetBoardDescription(_) | SetBoardTags(_) => "Edit document information",
        PlaceArtwork(_) => "Place artwork",
        InsertTracedPaths { .. } => "Trace image",
        Pathfinder(_) => "Pathfinder",
        InsertAnchor { .. } | AnchorType { .. } => "Edit anchors",
        LayerFamily { .. } => "Edit layers",
        // ---- w3-cmyk ----
        ColourManagement(crate::colour_management_commands::Command::Mode { .. }) => "Document colour mode",
        ColourManagement(crate::colour_management_commands::Command::Profile { .. }) => "Assign ICC profile",
        ColourManagement(crate::colour_management_commands::Command::Paint { .. }) => "Change source colour",
        _ => "Edit artwork",
    }
}
