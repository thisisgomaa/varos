//! Lane F: versioned semantic actions; selection is rebound by the caller at replay.
//! No persisted Rust enum discriminants, file operations, previews or session IDs.
use crate::{geom::Rgba, model::ShapeKind, EditCommand, Editor};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "id", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    #[serde(rename = "edit.nudge")]
    Nudge { delta_pt: [f32; 2] },
    #[serde(rename = "edit.opacity")]
    Opacity { value: f32 },
    #[serde(rename = "edit.paint")]
    Paint { fill: bool, colour: Option<Rgba> },
    #[serde(rename = "edit.stroke-width")]
    StrokeWidth { value: f32 },
    #[serde(rename = "edit.rotation")]
    Rotation { degrees: f32 },
    #[serde(rename = "edit.delete")]
    Delete {},
    #[serde(rename = "edit.group")]
    Group {},
    #[serde(rename = "edit.ungroup")]
    Ungroup {},
    #[serde(rename = "edit.create_rectangle")]
    Rectangle { local: String, bounds_pt: [f32; 4], fill: Option<Rgba> },
    #[serde(rename = "selection.local")]
    SelectLocal { local: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Actions {
    pub version: u32,
    pub name: String,
    pub steps: Vec<Step>,
}
impl Actions {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 1_048_576 {
            return Err("Actions exceed 1 MiB".into());
        }
        let a: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        a.validate()?;
        Ok(a)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported Actions version".into());
        }
        if self.steps.is_empty() || self.steps.len() > 100 || self.name.len() > 256 || self.name.trim().is_empty() {
            return Err("Use 1–100 steps and a name up to 256 bytes".into());
        }
        let mut locals = std::collections::BTreeSet::new();
        for step in &self.steps {
            match step {
                Step::Nudge { delta_pt } if !delta_pt.iter().all(|v| v.is_finite()) => {
                    return Err("Invalid points".into())
                }
                Step::Opacity { value } if !value.is_finite() || !(0.0..=1.0).contains(value) => {
                    return Err("Opacity must be 0–1".into())
                }
                Step::Paint { colour: Some(colour), .. }
                    if !colour.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) =>
                {
                    return Err("Invalid paint colour".into())
                }
                Step::StrokeWidth { value } if !value.is_finite() || *value < 0.0 => {
                    return Err("Stroke width must be finite and nonnegative".into())
                }
                Step::Rotation { degrees } if !degrees.is_finite() => return Err("Rotation must be finite".into()),
                Step::Rectangle { local, bounds_pt, fill } => {
                    if local.is_empty()
                        || local.len() > 64
                        || !locals.insert(local)
                        || !bounds_pt.iter().all(|v| v.is_finite())
                        || bounds_pt[2] <= 0.0
                        || bounds_pt[3] <= 0.0
                        || fill.is_some_and(|c| !c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)))
                    {
                        return Err("Invalid rectangle or duplicate local reference".into());
                    }
                }
                Step::SelectLocal { local } if !locals.contains(local) => return Err("Unbound local target".into()),
                _ => {}
            }
        }
        Ok(())
    }
    pub fn replay(&self, ed: &mut Editor) -> Result<(), String> {
        self.validate()?;
        let before = ed.rev;
        let targets = ed.objsel.iter().copied().collect::<Vec<_>>();
        let mut locals = std::collections::BTreeMap::new();
        let batch = ed.prepare_design_batch(
            self.steps.len(),
            |stage, index| {
                if index == 0 {
                    locals.clear();
                    stage.action_recording = None;
                    stage.try_execute(EditCommand::SelectPaths(targets.clone()))?;
                }
                match &self.steps[index] {
                    Step::Nudge { delta_pt } => {
                        if stage.objsel.is_empty() {
                            return Err("Bind a selection before replay".into());
                        }
                        stage
                            .execute_targeted_batch(vec![crate::bridge::TargetEdit::Move {
                                paths: stage.objsel.iter().copied().collect(),
                                delta: *delta_pt,
                            }])
                            .map_err(|e| e.reason)
                    }
                    Step::Opacity { value } => {
                        if stage.objsel.is_empty() {
                            return Err("Bind a selection before replay".into());
                        }
                        stage.try_execute(EditCommand::SetOpacity(*value))
                    }
                    Step::Paint { fill, colour } => stage.try_execute(EditCommand::ApplyPaint {
                        target: if *fill {
                            crate::editor::PaintTarget::Fill
                        } else {
                            crate::editor::PaintTarget::Stroke
                        },
                        color: *colour,
                    }),
                    Step::StrokeWidth { value } => stage.try_execute(EditCommand::SetStrokeWidth(*value)),
                    Step::Rotation { degrees } => stage.try_execute(EditCommand::SetObjectRotation(*degrees)),
                    Step::Delete {} => stage.try_execute(EditCommand::DeleteSelected),
                    Step::Group {} => stage.try_execute(EditCommand::GroupSelection),
                    Step::Ungroup {} => stage.try_execute(EditCommand::UngroupSelection),
                    Step::Rectangle { local, bounds_pt, fill } => {
                        let id = stage.try_execute_created(EditCommand::AddShape {
                            kind: ShapeKind::Rect,
                            bounds: *bounds_pt,
                            parent: None,
                            fill: *fill,
                            stroke: None,
                            stroke_width: 0.0,
                            opacity: 1.0,
                            name: None,
                        })?;
                        locals.insert(local.clone(), id);
                        Ok(())
                    }
                    Step::SelectLocal { local } => {
                        let id = *locals.get(local).ok_or("Unbound local target")?;
                        stage.try_execute(EditCommand::SelectPaths(vec![id]))
                    }
                }
            },
            |i, e| format!("Step {i}: {e}"),
            || false,
        )?;
        ed.publish_design_batch(batch)?;
        ed.annotate_history(before, crate::editor::history::Actor::Human, format!("Action: {}", self.name));
        Ok(())
    }
}
impl Editor {
    /// Called only after a typed host batch commits; failed batches never reach recording.
    pub fn record_action_batch(&mut self, before_rev: u64, steps: Result<Vec<Step>, String>) {
        if self.action_recording.is_none() || self.rev <= before_rev {
            return;
        }
        match steps {
            Ok(steps) => {
                for step in steps {
                    self.record_step(Some(step), before_rev);
                }
            }
            Err(reason) => {
                self.action_recording = None;
                self.action_recording_warning =
                    Some(format!("Recording stopped: {reason}; no partial action was saved"));
            }
        }
    }
    pub fn start_action_recording(&mut self) -> Result<(), String> {
        if self.transaction_open() {
            return Err("Finish the current edit".into());
        }
        if self.action_recording.is_some() {
            return Err("A recording is already active".into());
        }
        self.action_recording_warning = None;
        self.action_recording_targets = self.objsel.iter().copied().collect();
        self.action_recording_targets.sort_unstable();
        self.action_recording = Some(vec![]);
        Ok(())
    }
    pub(crate) fn refuse_action_recording(&mut self) {
        if self.action_recording.take().is_some() {
            self.action_recording_warning =
                Some("Unsupported document command stopped recording; no partial action was saved".into());
        }
    }
    pub(crate) fn check_action_targets(&mut self, targets: &[u32]) {
        let mut targets = targets.to_vec();
        targets.sort_unstable();
        if self.action_recording.as_ref().is_some_and(Vec::is_empty) && self.action_recording_targets.is_empty() {
            self.action_recording_targets = targets.clone();
        }
        if self.action_recording.is_some() && targets != self.action_recording_targets {
            self.action_recording = None;
            self.action_recording_warning =
                Some("Recording stopped: action targets changed; no partial action was saved".into());
        }
    }
    /// Publish covered Bridge metadata atomically with its document commit. Other batch callers
    /// remain unsupported and are caught by the same commit boundary as pointer gestures.
    pub fn publish_recorded_design_batch(
        &mut self,
        batch: crate::bridge::PreparedDesignBatch,
        targets: &[u32],
        steps: Result<Vec<Step>, String>,
    ) -> Result<(), String> {
        let before = self.rev;
        let recording = self.action_recording.clone();
        let warning = self.action_recording_warning.clone();
        let binding = self.action_recording_targets.clone();
        if !batch.document().content_eq(&self.doc) {
            self.check_action_targets(targets);
        }
        self.action_batch_covered = true;
        let result = self.publish_design_batch(batch);
        self.action_batch_covered = false;
        if result.is_err() {
            self.action_recording = recording;
            self.action_recording_warning = warning;
            self.action_recording_targets = binding;
        } else {
            self.record_action_batch(before, steps);
        }
        result
    }
    pub fn finish_action_recording(&mut self, name: String) -> Result<Actions, String> {
        let steps = self.action_recording.as_ref().ok_or("No recording is active")?.clone();
        let a = Actions { version: 1, name, steps };
        a.validate()?;
        self.action_recording = None;
        Ok(a)
    }
    pub fn cancel_action_recording(&mut self) {
        self.action_recording = None;
    }
    pub fn take_action_recording_warning(&mut self) -> Option<String> {
        self.action_recording_warning.take()
    }
    pub fn action_recording_len(&self) -> Option<usize> {
        self.action_recording.as_ref().map(Vec::len)
    }
    pub(crate) fn record_step(&mut self, step: Option<Step>, before: u64) {
        if self.rev > before && self.history_preview(false).is_some_and(|doc| !doc.content_eq(&self.doc)) {
            if self.action_recording.is_some() && step.is_none() {
                self.refuse_action_recording();
                return;
            }
            if let (Some(steps), Some(step)) = (&mut self.action_recording, step) {
                // Preserve overflow so Stop refuses instead of silently exporting an incomplete sequence.
                if steps.len() <= 100 {
                    steps.push(step);
                }
            }
        }
    }
}
pub(crate) fn semantic(command: &EditCommand) -> Option<Step> {
    match command {
        EditCommand::Nudge { x, y } => Some(Step::Nudge { delta_pt: [*x, *y] }),
        EditCommand::SetOpacity(value) => Some(Step::Opacity { value: *value }),
        EditCommand::ApplyPaint { target, color } => {
            Some(Step::Paint { fill: *target == crate::editor::PaintTarget::Fill, colour: *color })
        }
        EditCommand::SetStrokeWidth(value) => Some(Step::StrokeWidth { value: *value }),
        EditCommand::SetObjectRotation(degrees) => Some(Step::Rotation { degrees: *degrees }),
        EditCommand::DeleteSelected => Some(Step::Delete {}),
        EditCommand::GroupSelection => Some(Step::Group {}),
        EditCommand::UngroupSelection => Some(Step::Ungroup {}),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_ids_are_remapped_and_batch_is_atomic() {
        let a = Actions {
            version: 1,
            name: "Rect".into(),
            steps: vec![
                Step::Rectangle { local: "r".into(), bounds_pt: [0., 0., 10., 10.], fill: None },
                Step::SelectLocal { local: "r".into() },
                Step::Nudge { delta_pt: [3., 0.] },
            ],
        };
        let mut e = Editor::new();
        a.replay(&mut e).unwrap();
        assert_eq!(e.doc.paths.len(), 1);
        assert_eq!(e.history_depths(), (1, 0));
        let first = e.doc.paths[0].id;
        a.replay(&mut e).unwrap();
        assert_ne!(e.doc.paths[1].id, first);
        let bytes = crate::format::encode_model(&e.doc, &crate::format::Limits::DEFAULT).unwrap();
        let bad = Actions {
            version: 1,
            name: "bad".into(),
            steps: vec![
                Step::Rectangle { local: "r".into(), bounds_pt: [0., 0., 10., 10.], fill: None },
                Step::SelectLocal { local: "absent".into() },
            ],
        };
        assert!(bad.replay(&mut e).is_err());
        assert_eq!(bytes, crate::format::encode_model(&e.doc, &crate::format::Limits::DEFAULT).unwrap());
    }
    #[test]
    fn refusal_and_recording() {
        assert!(Actions::decode(br#"{"version":99,"name":"a","steps":[]}"#).is_err());
        let mut e = Editor::new();
        e.start_action_recording().unwrap();
        e.execute(EditCommand::Nudge { x: 1., y: 0. }).unwrap();
        assert_eq!(e.action_recording_len(), Some(0));
        assert!(e.finish_action_recording("Empty".into()).is_err());
    }
}

#[cfg(test)]
mod recorder_regressions {
    use super::*;
    fn rectangle(ed: &mut Editor) -> u32 {
        ed.try_execute_created(EditCommand::AddShape {
            kind: ShapeKind::Rect,
            bounds: [0., 0., 10., 10.],
            parent: None,
            fill: Some([1., 0., 0., 1.]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap()
    }
    #[test]
    fn committed_moves_record_noops_do_not_and_replay_is_one_step() {
        let mut ed = Editor::new();
        let id = rectangle(&mut ed);
        ed.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
        ed.start_action_recording().unwrap();
        assert!(ed.start_action_recording().is_err());
        ed.try_execute(EditCommand::Nudge { x: 0., y: 0. }).unwrap();
        assert_eq!(ed.action_recording_len(), Some(0));
        ed.try_execute(EditCommand::Nudge { x: 2., y: 3. }).unwrap();
        ed.try_execute(EditCommand::SetOpacity(0.5)).unwrap();
        ed.try_execute(EditCommand::SetOpacity(0.5)).unwrap();
        let action = ed.finish_action_recording("Move and fade".into()).unwrap();
        assert_eq!(action.steps.len(), 2);
        let before = ed.doc.clone();
        let depth = ed.history_depths().0;
        action.replay(&mut ed).unwrap();
        assert_eq!(ed.history_depths().0, depth + 1);
        ed.undo();
        assert!(ed.doc.content_eq(&before));
    }
    #[test]
    fn valid_preflight_can_still_fail_atomically_on_an_unbound_runtime_target() {
        let mut ed = Editor::new();
        let action = Actions {
            version: 1,
            name: "Unbound".into(),
            steps: vec![
                Step::Rectangle { local: "created".into(), bounds_pt: [0., 0., 10., 10.], fill: None },
                Step::Opacity { value: 0.5 },
            ],
        };
        assert!(action.validate().is_ok());
        assert!(action.replay(&mut ed).is_err());
        assert!(ed.doc.paths.is_empty());
        assert_eq!(ed.history_depths(), (0, 0));
    }
}

#[cfg(test)]
mod extended_steps {
    use super::*;
    #[test]
    fn recorded_paint_stroke_rotation_and_delete_roundtrip() {
        let mut ed = Editor::new();
        let seed = Actions {
            version: 1,
            name: "Seed".into(),
            steps: vec![Step::Rectangle { local: "r".into(), bounds_pt: [0., 0., 10., 10.], fill: None }],
        };
        seed.replay(&mut ed).unwrap();
        let id = ed.doc.paths[0].id;
        ed.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
        ed.start_action_recording().unwrap();
        ed.try_execute(EditCommand::ApplyPaint {
            target: crate::editor::PaintTarget::Fill,
            color: Some([0., 1., 0., 1.]),
        })
        .unwrap();
        ed.try_execute(EditCommand::SetStrokeWidth(3.)).unwrap();
        ed.try_execute(EditCommand::SetObjectRotation(30.)).unwrap();
        let recorded = ed.finish_action_recording("Style".into()).unwrap();
        assert_eq!(recorded.steps.len(), 3);
        let bytes = serde_json::to_vec(&recorded).unwrap();
        assert_eq!(Actions::decode(&bytes).unwrap(), recorded);
        let mut target = Editor::new();
        seed.replay(&mut target).unwrap();
        target.try_execute(EditCommand::SelectPaths(vec![target.doc.paths[0].id])).unwrap();
        recorded.replay(&mut target).unwrap();
        assert_eq!(target.doc.paths[0].fill, crate::model::Paint::Solid([0., 1., 0., 1.]));
        assert_eq!(target.doc.paths[0].stroke_width, 3.);
        let delete = Actions { version: 1, name: "Delete".into(), steps: vec![Step::Delete {}] };
        delete.replay(&mut target).unwrap();
        assert!(target.doc.paths.is_empty());
        target.undo();
        assert_eq!(target.doc.paths.len(), 1);
    }
}

#[cfg(test)]
mod fix_round_tests {
    use super::*;
    fn seed() -> (Editor, u32, u32) {
        let mut ed = Editor::new();
        let create = || EditCommand::AddShape {
            kind: ShapeKind::Rect,
            bounds: [0., 0., 10., 10.],
            parent: None,
            fill: Some([1., 0., 0., 1.]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        };
        let a = ed.try_execute_created(create()).unwrap();
        let b = ed.try_execute_created(create()).unwrap();
        ed.try_execute(EditCommand::SelectPaths(vec![a])).unwrap();
        (ed, a, b)
    }
    #[test]
    fn direct_gesture_commit_refuses_partial_recording() {
        let (mut ed, a, _) = seed();
        ed.start_action_recording().unwrap();
        ed.try_execute(EditCommand::Nudge { x: 1., y: 0. }).unwrap();
        assert_eq!(ed.action_recording_len(), Some(1));
        ed.begin();
        ed.doc.paths.iter_mut().find(|p| p.id == a).unwrap().opacity = 0.25;
        ed.dirty = true;
        ed.commit();
        assert!(ed.finish_action_recording("Partial".into()).is_err());
        assert!(ed.take_action_recording_warning().unwrap().contains("no partial"));
    }
    #[test]
    fn checked_creation_refuses_partial_recording() {
        let (mut ed, _, _) = seed();
        ed.start_action_recording().unwrap();
        ed.try_execute(EditCommand::Nudge { x: 1., y: 0. }).unwrap();
        ed.try_execute_created(EditCommand::AddShape {
            kind: ShapeKind::Rect,
            bounds: [0., 0., 10., 10.],
            parent: None,
            fill: None,
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
        assert!(ed.finish_action_recording("Partial".into()).is_err());
        assert!(ed.take_action_recording_warning().is_some());
    }
    #[test]
    fn changing_selection_refuses_recording_instead_of_rebinding_steps() {
        let (mut ed, _, b) = seed();
        ed.start_action_recording().unwrap();
        ed.try_execute(EditCommand::Nudge { x: 1., y: 0. }).unwrap();
        ed.try_execute(EditCommand::SelectPaths(vec![b])).unwrap();
        ed.try_execute(EditCommand::Nudge { x: 2., y: 0. }).unwrap();
        assert!(ed.finish_action_recording("Partial".into()).is_err());
        assert!(ed.take_action_recording_warning().unwrap().contains("targets changed"));
    }
    #[test]
    fn target_binding_survives_separate_bridge_publications() {
        let (mut ed, a, b) = seed();
        ed.start_action_recording().unwrap();
        for id in [a, b] {
            let batch = ed
                .prepare_design_batch(
                    1,
                    |stage, _| {
                        stage.try_execute(EditCommand::SelectPaths(vec![id]))?;
                        stage.try_execute(EditCommand::Nudge { x: 1., y: 0. })
                    },
                    |_, e| e,
                    || false,
                )
                .unwrap();
            ed.publish_recorded_design_batch(batch, &[id], Ok(vec![Step::Nudge { delta_pt: [1., 0.] }])).unwrap();
            if id == a {
                assert_eq!(ed.action_recording_len(), Some(1));
            }
        }
        assert!(ed.finish_action_recording("Partial".into()).is_err());
        assert!(ed.take_action_recording_warning().unwrap().contains("targets changed"));
    }
    #[test]
    fn direct_undo_refuses_partial_recording() {
        let (mut ed, _, _) = seed();
        ed.start_action_recording().unwrap();
        ed.try_execute(EditCommand::Nudge { x: 1., y: 0. }).unwrap();
        ed.undo();
        assert!(ed.finish_action_recording("Partial".into()).is_err());
        assert!(ed.take_action_recording_warning().is_some());
    }
    #[test]
    fn history_above_original_ceiling_is_refused() {
        let mut ed = Editor::new();
        assert!(ed.set_history_depth(200).is_ok());
        assert!(ed.set_history_depth(201).is_err());
        assert!(ed.set_history_depth(1000).is_err());
    }
}
