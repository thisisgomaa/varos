//! Lane F: provisional existing-kit sheets; owner design review pending.
use crate::app_command::{AppCommand, SessionId};
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
        self.effective = settings;
        self.generation = generation;
    }
    pub fn draw(&mut self, ctx: &egui::Context, commands: &mut Vec<AppCommand>, sid: Option<SessionId>) {
        let Some(sheet) = self.sheet else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Area::new(egui::Id::new("lane-f-sheet")).order(egui::Order::Foreground).fixed_pos(ctx.content_rect().center()-egui::vec2(t::DOC_SHEET_W/2.0,t::DOC_SHEET_TOP)).show(ctx,|ui|{egui::Frame::new().fill(t::PANEL).stroke(egui::Stroke::new(t::KIT_STROKE,t::LINE2)).corner_radius(t::r_box()).inner_margin(egui::Margin::same(t::KIT_PAD as i8)).show(ui,|ui|{
            ui.set_width(t::DOC_SHEET_W);ui.label(egui::RichText::new(match sheet{DesktopAction::Preferences=>"Preferences",DesktopAction::Shortcuts=>"Keyboard Shortcuts",DesktopAction::Actions=>"Actions",DesktopAction::Help=>"Varos Help",DesktopAction::ReportProblem=>"Report a problem"}).font(t::small()).color(t::TEXT));
            egui::ScrollArea::vertical().max_height(ctx.content_rect().height()*0.65).show(ui,|ui|match sheet{
                DesktopAction::Preferences=>{if let Some((draft,generation))=&mut self.draft{
                    let mut category="";for spec in preferences::SPECS{if category!=spec.category{category=spec.category;kit::separator(ui);ui.label(egui::RichText::new(category).font(t::small()).color(t::MUTED));}
                        ui.horizontal(|ui|{ui.label(egui::RichText::new(spec.label).font(t::small()).color(t::TEXT));let current=spec.value(draft);let id=ui.id().with(spec.key);let change=match spec.control{
                            SettingControl::Boolean=>{let b=current.as_bool().unwrap_or(false);if kit::menu_row(ui,Control::new(id,if b{"On"}else{"Off"})).activated{Some(serde_json::json!(!b))}else{None}},
                            SettingControl::Choice(choices)=>kit::text_dropdown(ui,id,current.as_str().unwrap_or_default(),choices,t::DOC_SHEET_FIELD_W,spec.timing).map(|i|serde_json::json!(choices[i])),
                            _=>{let value=current.as_str().map(str::to_owned).unwrap_or_else(||current.to_string());let (rect,_)=ui.allocate_exact_size(egui::vec2(t::DOC_SHEET_FIELD_W,t::FIELD_H),egui::Sense::hover());let edit=kit::field::text_field(ui,kit::field::TextField{id,rect,value:&value,font:t::small(),framed:true,open:false,hint:""},|text|{let v=if matches!(spec.control,SettingControl::Colour){serde_json::json!(text)}else{if matches!(spec.typed,preferences::Key::Increment){serde_json::json!(varos_core::units::parse_to_pt(text,varos_core::DocUnits{ppi:72.0,display:draft.preferences.default_units.core()}).ok_or("Enter a length")?)}else{serde_json::from_str(text).map_err(|_|"Enter a number")?}};let mut copy=*draft;spec.set(&mut copy,v.clone()).map_err(|_|"Value outside supported range")?;Ok(v)});edit.commit}
                        };if let Some(v)=change{if let Err(e)=spec.set(draft,v){self.error=Some(e);}}});ui.label(egui::RichText::new(spec.timing).font(t::small()).color(t::MUTED));
                    }
                    ui.horizontal(|ui|{if kit::menu_row(ui,Control::new(ui.id().with("reset"),"Reset to Defaults")).activated{*draft=Settings::default();self.reset_requested=true;}if kit::menu_row(ui,Control::new(ui.id().with("apply"),"Apply")).activated{if preferences::validate(draft).is_ok() && !kit::field::any_open(ctx){commands.push(AppCommand::ApplyPreferences(*draft,*generation,self.reset_requested));}else{self.error=Some("Finish valid field edits before Apply".into());}}});
                }},
                DesktopAction::Shortcuts=>self.shortcuts.draw(ui,&mut self.error,commands),
                DesktopAction::Actions=>{ui.label(egui::RichText::new("Records committed nudges and opacity changes. Replay binds the current selection.").font(t::small()).color(t::MUTED));if let Some(sid)=sid{for (label,start) in [("Start Recording",true),("Stop Recording",false)]{if kit::menu_row(ui,Control::new(ui.id().with(label),label)).activated{commands.push(AppCommand::RecordAction(sid,start));}}if kit::menu_row(ui,Control::new(ui.id().with("replay"),"Replay")).activated{if let Some(a)=&self.actions{commands.push(AppCommand::ReplayAction(sid,a.clone()));}}}if let Some(a)=&self.actions{ui.label(egui::RichText::new(format!("{} · {} steps",a.name,a.steps.len())).font(t::small()).color(t::TEXT));if kit::menu_row(ui,Control::new(ui.id().with("save"),"Save .vrs-actions…")).activated{commands.push(AppCommand::SaveAction(a.clone()));}}},
                _=>{}
            });
            if let Some(error)=&self.error{ui.label(egui::RichText::new(error).font(t::small()).color(t::TEXT));}
            if kit::menu_row(ui,Control::new(ui.id().with("cancel"),"Close / Cancel")).activated{close=true;}
        });});
        if close {
            self.sheet = None;
            self.draft = None;
        }
    }
}
pub fn history(ui: &mut egui::Ui, ed: &varos_core::Editor, sid: Option<SessionId>, commands: &mut Vec<AppCommand>) {
    let Some(sid) = sid else { return };
    let depth = ed.history_depths().0;
    if depth == 0 {
        ui.label(egui::RichText::new("No undo steps").font(t::small()).color(t::MUTED));
    }
    if kit::menu_row(ui, Control::new(ui.id().with("initial"), "Initial state")).activated {
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
            label.push_str(" · undone");
        }
        if kit::menu_row(ui, Control::new(ui.id().with(("history", entry.rev_after)), &label)).activated {
            commands.push(AppCommand::HistoryJump(sid, i + 1));
        }
    }
    let _ = ed.history_preview(false);
}
