//! Lane C: validated atomic colour-management edits, shared by all hosts.
use crate::{
    colour_commands::ColourCommand,
    colour_management::{ColourMode, IccProfile, ManagedColour},
    editor::{Editor, PaintTarget},
    model::Paint,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Proof { enabled: bool },
    Overprint { enabled: bool },
    Mode { mode: ColourMode },
    Profile { profile: Option<IccProfile> },
    Paint { target: PaintTarget, colour: ManagedColour },
}
pub fn check(ed: &Editor, c: &Command) -> Result<(), String> {
    match c {
        Command::Proof { enabled: true } => {
            let profile = ed.doc.output_profile.as_ref().ok_or("Choose an ICC output profile before Proof Colours")?;
            crate::colour_preview::proof_rgb([0.5, 0.5, 0.5, 1.], profile)?;
            Ok(())
        }
        Command::Profile { profile: Some(p) } => {
            let mut doc = ed.doc.clone();
            doc.output_profile = Some(p.clone());
            crate::colour_management::validate_document(&doc)
        }
        Command::Paint { target, colour } => {
            crate::colour_commands::check(
                ed,
                &ColourCommand::Paint { target: *target, paint: Paint::Managed(colour.clone()) },
            )?;
            if let crate::colour_management::Colour::Spot { name, alt, .. } = &colour.colour {
                let selected = ed.selected_pids();
                let mut other = ed
                    .doc
                    .paths
                    .iter()
                    .flat_map(|p| [(p.id, PaintTarget::Fill, &p.fill), (p.id, PaintTarget::Stroke, &p.stroke)])
                    .filter(|(id, slot, _)| !selected.contains(id) || slot != target)
                    .map(|(_, _, paint)| paint)
                    .chain(ed.doc.swatches.iter().map(|s| &s.paint));
                if other.any(|p| matches!(p,Paint::Managed(ManagedColour {colour:crate::colour_management::Colour::Spot {name: n,alt:a,..},..}) if n==name && a!=alt)) {
                    return Err("a spot name must have one consistent alternate colour".into());
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
pub fn apply(ed: &mut Editor, c: Command) {
    if check(ed, &c).is_err() {
        return;
    }
    match c {
        Command::Proof { enabled } => ed.colour_preview.proof = enabled,
        Command::Overprint { enabled } => ed.colour_preview.overprint = enabled,
        Command::Paint { target, colour } => {
            crate::colour_commands::apply(ed, ColourCommand::Paint { target, paint: Paint::Managed(colour) })
        }
        Command::Mode { mode } => {
            ed.begin();
            ed.doc.colour_mode = mode;
            ed.finish_document_setup();
        }
        Command::Profile { profile } => {
            ed.begin();
            if profile.is_none() {
                ed.colour_preview.proof = false;
            }
            ed.doc.output_profile = profile;
            ed.finish_document_setup();
        }
    }
}
