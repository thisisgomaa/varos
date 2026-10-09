//! Lane F: provisional existing-kit sheets; owner design review pending.
use crate::app_command::{AppCommand, SessionId};
use varos_app::shell::kit::text::ShapedUi as _;
use varos_app::{
    shell::{
        kit::{self, Control},
        tokens as t,
    },
    storage::{
        preferences::{self, Control as SettingControl},
        settings::Settings,
    },
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopAction {
    Preferences,
    Shortcuts,
    Actions,
    Help,
    ReportProblem,
}
#[derive(Default)]
pub struct State {
    pub effective: Settings,
    reset_requested: bool,
    pub generation: u64,
    pub sheet: Option<DesktopAction>,
    draft: Option<(Settings, u64)>,
    pub error: Option<String>,
    pub recording: bool,
    pub gpu_effective: String,
    pub history_depths: Vec<(usize, usize)>,
    launch_gpu: Option<preferences::GpuPreference>,
    launch_language: Option<varos_app::i18n::Locale>,
    pub shortcuts: crate::shortcut_editor::EditorState,
    pub actions: Option<varos_core::actions::Actions>,
}
impl State {
    pub fn open(&mut self, action: DesktopAction) {
        self.sheet = Some(action);
        self.error = None;
        self.reset_requested = false;
        if action == DesktopAction::Shortcuts {
            self.shortcuts.draft = None;
        }
        if action == DesktopAction::Preferences {
            self.draft = Some((self.effective, self.generation));
        }
    }
    pub fn sync(&mut self, settings: Settings, generation: u64) {
        if self.draft.is_some_and(|(draft, old)| old != generation && draft == settings) {
            self.sheet = None;
            self.draft = None;
        }
        self.launch_gpu.get_or_insert(settings.preferences.gpu_preference);
        self.effective = settings;
        self.generation = generation;
    }
    pub fn sync_shortcuts(&mut self, overrides: crate::shortcut_editor::Overrides, generation: u64) {
        if self.shortcuts.draft.as_ref() == Some(&overrides) && self.shortcuts.draft_generation != generation {
            self.sheet = None;
            self.shortcuts.draft = None;
        }
        self.shortcuts.effective = overrides;
        self.shortcuts.generation = generation;
    }
    pub fn draw(&mut self, ctx: &egui::Context, commands: &mut Vec<AppCommand>, sid: Option<SessionId>) {
        static SYSTEM: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let system = SYSTEM.get_or_init(varos_app::i18n::system_locale);
        let locale = *self
            .launch_language
            .get_or_insert_with(|| varos_app::i18n::resolve(self.effective.preferences.language.requested(), system));
        varos_app::i18n::set(ctx, locale);
        let Some(sheet) = self.sheet else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !kit::field::any_open(ctx);
        egui::Area::new(egui::Id::new("lane-f-sheet"))
            .order(egui::Order::Foreground)
            .fixed_pos(ctx.content_rect().center() - egui::vec2(t::DOC_SHEET_W / 2.0, t::DOC_SHEET_TOP))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t::PANEL)
                    .stroke(egui::Stroke::new(t::KIT_STROKE, t::LINE2))
                    .corner_radius(t::r_box())
                    .inner_margin(egui::Margin::same(t::KIT_PAD as i8))
                    .show(ui, |ui| {
                        ui.set_width(t::DOC_SHEET_W);
                        let title = match sheet {
                            DesktopAction::Preferences => "Preferences",
                            DesktopAction::Shortcuts => "Keyboard Shortcuts",
                            DesktopAction::Actions => "Actions",
                            DesktopAction::Help => "Varos Help",
                            DesktopAction::ReportProblem => "Report a problem",
                        };
                        ui.shaped_label(egui::RichText::new(title).font(t::small()).color(t::TEXT));
                        egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.65).show(ui, |ui| {
                            match sheet {
                                DesktopAction::Preferences => self.preferences(ui, commands),
                                DesktopAction::Shortcuts => self.shortcuts.draw(ui, &mut self.error, commands),
                                DesktopAction::Actions => self.actions(ui, commands, sid),
                                _ => {}
                            }
                        });
                        if let Some(error) = &self.error {
                            ui.shaped_label(egui::RichText::new(error).font(t::small()).color(t::TEXT));
                        }
                        if kit::menu_row(ui, Control::new(ui.id().with("reconcile"), "Reconcile disk before retry"))
                            .activated
                        {
                            commands.push(AppCommand::ReconcilePreferences);
                        }
                        if kit::menu_row(ui, Control::new(ui.id().with("cancel"), "Cancel / Close")).activated {
                            close = true;
                        }
                    });
            });
        if close {
            self.sheet = None;
            self.draft = None;
        }
    }
    fn preferences(&mut self, ui: &mut egui::Ui, commands: &mut Vec<AppCommand>) {
        let Some((draft, generation)) = &mut self.draft else { return };
        let mut category = "";
        for spec in preferences::SPECS {
            if category != spec.category {
                category = spec.category;
                kit::separator(ui);
                ui.shaped_label(egui::RichText::new(category).font(t::small()).color(t::MUTED));
            }
            ui.horizontal(|ui| {
                ui.shaped_label(egui::RichText::new(spec.label).font(t::small()).color(t::TEXT));
                let current = spec.value(draft);
                let id = ui.id().with(spec.key);
                let enabled = !matches!(spec.typed, preferences::Key::Interval) || draft.autosave_enabled;
                ui.add_enabled_ui(enabled, |ui| {
                    let change = match spec.control {
                        SettingControl::Boolean => {
                            let b = current.as_bool().unwrap_or(false);
                            kit::menu_row(ui, Control::new(id, if b { "On" } else { "Off" }))
                                .activated
                                .then(|| serde_json::json!(!b))
                        }
                        SettingControl::Choice(choices) => kit::text_dropdown(
                            ui,
                            id,
                            current.as_str().unwrap_or_default(),
                            choices,
                            t::DOC_SHEET_FIELD_W,
                            spec.timing,
                        )
                        .map(|i| serde_json::json!(choices[i])),
                        _ => {
                            let value = current.as_str().map(str::to_owned).unwrap_or_else(|| current.to_string());
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(t::DOC_SHEET_FIELD_W, t::FIELD_H),
                                egui::Sense::hover(),
                            );
                            kit::field::text_field(
                                ui,
                                kit::field::TextField {
                                    id,
                                    rect,
                                    value: &value,
                                    font: t::small(),
                                    framed: true,
                                    open: false,
                                    hint: "",
                                },
                                |text| {
                                    let v = match spec.typed {
                                        preferences::Key::Increment => {
                                            serde_json::json!(varos_core::units::parse_to_pt(
                                                text,
                                                varos_core::DocUnits {
                                                    ppi: 72.0,
                                                    display: varos_core::units::Unit::Pt
                                                }
                                            )
                                            .ok_or("Enter a length in pt, px, pc, mm, cm or in")?)
                                        }
                                        preferences::Key::Canvas => serde_json::json!(text),
                                        _ => serde_json::from_str(text).map_err(|_| "Enter a number")?,
                                    };
                                    let mut copy = *draft;
                                    spec.set(&mut copy, v.clone()).map_err(|_| "Value outside supported range")?;
                                    Ok(v)
                                },
                            )
                            .commit
                        }
                    };
                    if let Some(v) = change {
                        if let Err(e) = spec.set(draft, v) {
                            self.error = Some(e);
                        }
                    }
                });
            });
            ui.shaped_label(egui::RichText::new(spec.timing).font(t::small()).color(t::MUTED));
            if matches!(spec.typed, preferences::Key::History) {
                let discarded = self
                    .history_depths
                    .iter()
                    .map(|(undo, redo)| {
                        undo.saturating_sub(draft.preferences.history_depth)
                            + redo.saturating_sub(draft.preferences.history_depth)
                    })
                    .sum::<usize>();
                if discarded > 0 {
                    ui.shaped_label(
                        egui::RichText::new(varos_app::i18n::message(
                            ui.ctx(),
                            "Apply discards {discarded} retained history steps",
                            &[("discarded", &discarded.to_string())],
                        ))
                        .font(t::small())
                        .color(t::TEXT),
                    );
                }
            }
            if matches!(spec.typed, preferences::Key::Language)
                && matches!(draft.preferences.language, preferences::Language::Unavailable { .. })
                && varos_app::i18n::Locale::from_tag(draft.preferences.language.requested())
                    == varos_app::i18n::Locale::En
            {
                ui.shaped_label(
                    egui::RichText::new(
                        "Requested catalog unavailable; English fallback. Choose System or English to change it.",
                    )
                    .font(t::small())
                    .color(t::MUTED),
                );
            }
            if matches!(spec.typed, preferences::Key::Gpu) {
                ui.shaped_label(
                    egui::RichText::new(varos_app::i18n::message(
                        ui.ctx(),
                        "Active adapter: {adapter}",
                        &[("adapter", &self.gpu_effective)],
                    ))
                    .font(t::small())
                    .color(t::MUTED),
                );
                ui.shaped_label(
                    egui::RichText::new(varos_app::i18n::message(
                        ui.ctx(),
                        "Effective launch hint: {effective}; requested: {requested}. Restart required.",
                        &[
                            (
                                "effective",
                                &format!("{:?}", self.launch_gpu.unwrap_or(self.effective.preferences.gpu_preference)),
                            ),
                            ("requested", &format!("{:?}", draft.preferences.gpu_preference)),
                        ],
                    ))
                    .font(t::small())
                    .color(t::MUTED),
                );
            }
        }
        ui.horizontal(|ui| {
            if kit::menu_row(ui, Control::new(ui.id().with("reset"), "Reset to Defaults")).activated {
                let paste_remembers_layers = draft.paste_remembers_layers;
                *draft = Settings::default();
                draft.paste_remembers_layers = paste_remembers_layers;
                self.reset_requested = true;
            }
            if kit::menu_row(ui, Control::new(ui.id().with("apply"), "Apply")).activated {
                if preferences::validate(draft).is_ok() && !kit::field::any_open(ui.ctx()) {
                    commands.push(AppCommand::ApplyPreferences(*draft, *generation, self.reset_requested));
                } else {
                    self.error = Some("Finish valid field edits before Apply".into());
                }
            }
        });
    }
    pub(crate) fn actions(&mut self, ui: &mut egui::Ui, commands: &mut Vec<AppCommand>, sid: Option<SessionId>) {
        ui.shaped_label(egui::RichText::new("Records committed moves, paint, opacity, stroke width, rotation, delete, group and ungroup. Replay binds the current selection; file and UI commands are excluded.").font(t::small()).color(t::MUTED));
        if let Some(sid) = sid {
            if self.recording
                && kit::menu_row(ui, Control::new(ui.id().with("discard-recording"), "Discard Recording")).activated
            {
                commands.push(AppCommand::CancelActionRecording(sid));
            }
            if kit::menu_row(
                ui,
                Control::new(ui.id().with("record"), if self.recording { "Stop Recording" } else { "Start Recording" }),
            )
            .activated
            {
                commands.push(AppCommand::RecordAction(sid, !self.recording));
            }
            if let Some(a) = &self.actions {
                if kit::menu_row(ui, Control::new(ui.id().with("replay"), "Replay")).activated {
                    commands.push(AppCommand::ReplayAction(sid, a.clone()));
                }
            }
        } else {
            ui.shaped_label(
                egui::RichText::new("Open a document to record or replay").font(t::small()).color(t::MUTED),
            );
        }
        if kit::menu_row(ui, Control::new(ui.id().with("load"), "Load .vrs-actions…")).activated {
            commands.push(AppCommand::LoadAction);
        }
        if let Some(a) = &self.actions {
            ui.shaped_label(
                egui::RichText::new(varos_app::i18n::message(
                    ui.ctx(),
                    "{name} · {count} steps",
                    &[("name", &a.name), ("count", &a.steps.len().to_string())],
                ))
                .font(t::small())
                .color(t::TEXT),
            );
            if kit::menu_row(ui, Control::new(ui.id().with("save"), "Save .vrs-actions…")).activated {
                commands.push(AppCommand::SaveAction(a.clone()));
            }
        }
    }
}

pub fn history(ui: &mut egui::Ui, ed: &varos_core::Editor, sid: Option<SessionId>, commands: &mut Vec<AppCommand>) {
    let Some(sid) = sid else { return };
    let depth = ed.history_depths().0;
    if depth == 0 {
        ui.shaped_label(egui::RichText::new("No undo steps").font(t::small()).color(t::MUTED));
    }
    if kit::menu_row(ui, Control::new(ui.id().with("initial"), "Earliest retained state")).activated {
        commands.push(AppCommand::HistoryJump(sid, 0));
    }
    for (i, entry) in ed
        .history_entries()
        .iter()
        .chain(ed.history_redo_entries().iter().rev())
        .enumerate()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let mut label = match &entry.actor {
            varos_core::editor::history::Actor::Human => entry.label.clone(),
            varos_core::editor::history::Actor::Agent { label, .. } => format!("{label} · {}", entry.label),
        };
        if i >= depth {
            label = varos_app::i18n::message(ui.ctx(), "{label} · undone", &[("label", &label)]);
        }
        let mut row = Control::new(ui.id().with(("history", entry.rev_after)), &label);
        row.selected = i + 1 == depth;
        if kit::menu_row(ui, row).activated {
            commands.push(AppCommand::HistoryJump(sid, i + 1));
        }
        let colour =
            if matches!(entry.actor, varos_core::editor::history::Actor::Agent { .. }) { t::AGENT } else { t::MUTED };
        ui.shaped_label(
            egui::RichText::new(varos_app::i18n::message(
                ui.ctx(),
                "{created} created · {changed} changed · {removed} removed",
                &[
                    ("created", &entry.summary.created.to_string()),
                    ("changed", &entry.summary.changed.to_string()),
                    ("removed", &entry.summary.removed.to_string()),
                ],
            ))
            .font(t::small())
            .color(colour),
        );
    }
}
