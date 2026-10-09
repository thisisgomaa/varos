//! Lane F: draft generation and disk compare before FIFO durable publication.
use super::*;
impl RecoveryHost {
    pub(super) fn bridge_shortcuts(
        &mut self,
        v: &varos_bridge::application::ShortcutsRequest,
    ) -> Result<varos_bridge::Reply, varos_bridge::Error> {
        use varos_bridge::{application::ShortcutAction as A, Error, Reply};
        let (generation, overrides) = match &v.action {
            A::Read {} => {
                return Ok(Reply::success(
                    serde_json::json!({"generation":self.preferences_generation,"overrides":self.shortcuts}),
                ))
            }
            A::Apply { expected_generation, bindings } => {
                let mut o = self.shortcuts.clone();
                for (id, chord) in bindings {
                    o.bindings.insert(
                        id.clone(),
                        chord.as_ref().map(|c| crate::shortcut_editor::Chord {
                            key: c.key.clone(),
                            primary: c.primary,
                            shift: c.shift,
                            alt: c.alt,
                        }),
                    );
                }
                (*expected_generation, o)
            }
            A::Reset { expected_generation } => {
                let mut o = self.shortcuts.clone();
                for c in crate::command_registry::commands() {
                    o.bindings.insert(c.id, c.accel.map(crate::shortcut_editor::Chord::from_accel));
                }
                (*expected_generation, o)
            }
        };
        if generation != self.preferences_generation {
            return Err(Error::new("busy", "Preferences generation changed"));
        }
        overrides.validate().map_err(|e| Error::new("invalid_argument", e))?;
        self.apply_shortcuts(overrides);
        Ok(Reply::success(serde_json::json!({"accepted":true,"generation":self.preferences_generation})))
    }
    pub(super) fn bridge_preferences(
        &mut self,
        v: &varos_bridge::application::Preferences,
    ) -> Result<varos_bridge::Reply, varos_bridge::Error> {
        use varos_bridge::{application::PreferenceAction as A, Error, Reply};
        match &v.action {
            A::Read {} => Ok(Reply::success(
                serde_json::json!({"generation":self.preferences_generation,"settings":self.settings,"pending":self.preferences_pending,"warning":self.warning}),
            )),
            A::Apply { expected_generation, values } => {
                let value = serde_json::to_value(values).map_err(|e| Error::new("invalid_argument", e.to_string()))?;
                let mut next = self.settings;
                for spec in varos_app::storage::preferences::SPECS {
                    spec.set(&mut next, value[spec.key].clone()).map_err(|e| Error::new("invalid_argument", e))?;
                }
                if *expected_generation != self.preferences_generation || self.preferences_pending {
                    return Err(Error::new("busy", "Settings generation changed or write pending"));
                }
                self.apply_preferences(next, *expected_generation, false);
                if !self.preferences_pending {
                    return Err(Error::new("unsupported", self.warning.clone().unwrap_or_default()));
                }
                Ok(Reply::success(
                    serde_json::json!({"accepted":true,"generation":self.preferences_generation,"pending":true}),
                ))
            }
        }
    }

    pub(super) fn apply_preferences(&mut self, settings: Settings, generation: u64, reset: bool) -> bool {
        let refuse = if self.preferences_pending {
            Some("Preferences are being saved")
        } else if generation != self.preferences_generation {
            Some("Preferences changed; close and reopen the draft")
        } else {
            None
        };
        if let Some(reason) = refuse {
            self.warning = Some(reason.into());
            self.changed = true;
            return true;
        }
        if let Err(e) = varos_app::storage::preferences::validate(&settings) {
            self.warning = Some(e);
            self.changed = true;
            return true;
        }
        let (Some(path), Some(worker)) = (self.settings_evidence_path.clone(), &self.worker) else {
            self.warning = Some("Preferences cannot be saved; settings file is unavailable or locked".into());
            self.changed = true;
            return true;
        };
        let expected = self.settings_source.clone();
        self.warning = None;
        let job = Box::new(move || {
            let result = varos_app::storage::settings_codec::write_explicit(
                &RealFs,
                &path,
                &settings,
                expected.as_deref(),
                reset,
            )
            .and_then(|(outcome, bytes)| match outcome {
                WriteOutcome::Durable => Ok(bytes),
                WriteOutcome::ReplacedUnconfirmed(e) => {
                    Err(format!("Settings bytes replaced; durability unconfirmed. Reconcile disk before retry: {e}"))
                }
            });
            Finished::Preferences(settings, result)
        });
        self.preferences_pending = true;
        if worker.submit(job, Finished::Preferences(settings, Err("Settings worker failed".into()))).is_err() {
            self.preferences_pending = false;
            self.warning = Some("Settings worker stopped".into());
        }
        true
    }
    pub(super) fn apply_shortcuts(&mut self, overrides: crate::shortcut_editor::Overrides) -> bool {
        if let Err(e) = overrides.validate() {
            self.warning = Some(e);
            return true;
        }
        if let Some(worker) = &self.worker {
            let copy = overrides.clone();
            let job = Box::new(move || {
                let result = copy.save();
                Finished::Shortcuts(copy, result)
            });
            if worker.submit(job, Finished::Shortcuts(overrides, Err("Shortcut worker failed".into()))).is_err() {
                self.warning = Some("Shortcut worker stopped".into());
            }
        }
        true
    }
}
