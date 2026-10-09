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
        if generation != self.preferences_generation || self.preferences_pending {
            return Err(Error::new("busy", "Preferences generation changed"));
        }
        overrides.validate().map_err(|e| Error::new("invalid_argument", e))?;
        self.apply_shortcuts(overrides, generation);
        if !self.preferences_pending {
            return Err(Error::new("unsupported", self.warning.clone().unwrap_or_default()));
        }
        Ok(Reply::success(serde_json::json!({"accepted":true,"generation":self.preferences_generation,"pending":true})))
    }
    pub(super) fn bridge_preferences(
        &mut self,
        v: &varos_bridge::application::Preferences,
    ) -> Result<varos_bridge::Reply, varos_bridge::Error> {
        use varos_bridge::{application::PreferenceAction as A, Error, Reply};
        match &v.action {
            A::Reconcile {} => {
                if self.preferences_pending {
                    return Err(Error::new("busy", "Wait for the pending settings write"));
                }
                self.reconcile_preferences();
                if !self.preferences_pending {
                    return Err(Error::new("unsupported", self.warning.clone().unwrap_or_default()));
                }
                Ok(Reply::success(serde_json::json!({"accepted":true,"pending":true})))
            }
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
    pub(super) fn apply_shortcuts(&mut self, overrides: crate::shortcut_editor::Overrides, generation: u64) -> bool {
        self.changed = true;
        if generation != self.preferences_generation || self.preferences_pending {
            self.warning = Some("Preferences changed or a write is pending; reopen the draft".into());
            return true;
        }
        if let Err(e) = overrides.validate() {
            self.warning = Some(e);
            return true;
        }
        let (Some(path), Some(worker)) = (self.shortcuts_path.clone(), &self.worker) else {
            self.warning = Some("Shortcut writer unavailable".into());
            return true;
        };
        let expected = self.shortcuts_source.clone();
        let job = Box::new(move || Finished::Shortcuts(overrides.save_at(&RealFs, &path, expected.as_deref())));
        self.preferences_pending = true;
        if worker.submit(job, Finished::Shortcuts(Err("Shortcut worker failed".into()))).is_err() {
            self.preferences_pending = false;
            self.warning = Some("Shortcut worker stopped".into());
        }
        true
    }
    pub(super) fn reconcile_preferences(&mut self) -> bool {
        self.changed = true;
        if self.preferences_pending {
            self.warning = Some("Wait for the pending write".into());
            return true;
        }
        let (Some(path), Some(shortcuts), Some(worker)) =
            (self.settings_evidence_path.clone(), self.shortcuts_path.clone(), &self.worker)
        else {
            self.warning = Some("Settings writer unavailable".into());
            return true;
        };
        let job = Box::new(move || {
            fn read(path: &std::path::Path) -> Result<Option<Vec<u8>>, String> {
                match RealFs.read_limited(path, varos_app::storage::settings_codec::MAX_BYTES) {
                    Ok(bytes) if bytes.len() <= 65536 => Ok(Some(bytes)),
                    Ok(_) => Err("Settings or shortcuts exceed 64 KiB".into()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(e.to_string()),
                }
            }
            Finished::Reconciled(read(&path).and_then(|settings| Ok((settings, read(&shortcuts)?))))
        });
        self.preferences_pending = true;
        if worker.submit(job, Finished::Reconciled(Err("Settings worker failed".into()))).is_err() {
            self.preferences_pending = false;
            self.warning = Some("Settings worker stopped".into());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    struct TestDir(PathBuf);
    impl TestDir {
        fn new(name: &str) -> Self {
            Self(std::env::temp_dir().join(format!("{name}-{}", varos_app::storage::checksum::new_nonce())))
        }
        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn rig() -> (TestDir, RecoveryHost, Workspace, mpsc::Receiver<()>) {
        let dir = TestDir::new("lane-f-settings-host");
        let (tx, rx) = mpsc::channel();
        let host = RecoveryHost::at(
            Some(AppLayout { root: dir.join("data") }),
            Box::new(move || {
                let _ = tx.send(());
            }),
        );
        (dir, host, Workspace::new(), rx)
    }
    fn land(host: &mut RecoveryHost, ws: &mut Workspace, rx: &mpsc::Receiver<()>) {
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        host.observe(ws, Instant::now());
    }
    #[test]
    fn durable_toggle_then_draft_and_stale_generation_are_serialized() {
        let (_dir, mut host, mut ws, rx) = rig();
        host.handle(&AppCommand::SetRecoveryEnabled(false), &mut ws, Instant::now());
        assert!(host.settings.recovery_enabled);
        let mut next = host.settings;
        next.preferences.keyboard_increment_pt = 2.0;
        host.apply_preferences(next, 0, false);
        assert_eq!(host.preferences_generation, 0);
        land(&mut host, &mut ws, &rx);
        assert!(!host.settings.recovery_enabled);
        next.recovery_enabled = false;
        host.apply_preferences(next, 0, false);
        assert!(!host.preferences_pending);
        host.apply_preferences(next, 1, false);
        land(&mut host, &mut ws, &rx);
        assert_eq!(host.preferences_generation, 2);
        assert_eq!(ws.active().unwrap().editor.keyboard_increment_pt, 2.0);
        assert!(!settings::load(&RealFs, host.settings_evidence_path.as_ref().unwrap()).0.recovery_enabled);
        host.shutdown();
    }
    #[test]
    fn external_replacement_requires_explicit_reconciliation_and_retry() {
        let (_dir, mut host, mut ws, rx) = rig();
        let path = host.settings_evidence_path.clone().unwrap();
        std::fs::write(&path, br#"{"version":2,"future_key":{"nested":42}}"#).unwrap();
        let mut next = host.settings;
        next.preferences.history_depth = 5;
        host.apply_preferences(next, 0, false);
        land(&mut host, &mut ws, &rx);
        assert_eq!(host.preferences_generation, 0);
        assert_eq!(host.settings.preferences.history_depth, 200);
        assert!(host.warning.as_ref().unwrap().contains("changed on disk"));
        host.reconcile_preferences();
        land(&mut host, &mut ws, &rx);
        assert_eq!(host.preferences_generation, 0);
        host.apply_preferences(next, 0, false);
        land(&mut host, &mut ws, &rx);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["future_key"]["nested"], 42);
        assert_eq!(host.settings.preferences.history_depth, 5);
        host.shutdown();
    }
    #[test]
    fn shortcut_publication_is_generation_checked_and_blocks_settings_races() {
        let (_dir, mut host, mut ws, rx) = rig();
        let mut overrides = crate::shortcut_editor::Overrides::default();
        overrides.bindings.insert("edit.copy".into(), None);
        host.apply_shortcuts(overrides.clone(), 0);
        host.apply_preferences(Settings::default(), 0, false);
        assert!(host.shortcuts.bindings.is_empty());
        land(&mut host, &mut ws, &rx);
        assert_eq!(host.shortcuts, overrides);
        assert_eq!(host.preferences_generation, 1);
        host.apply_shortcuts(crate::shortcut_editor::Overrides::default(), 0);
        assert!(!host.preferences_pending);
        host.shutdown();
    }
}
