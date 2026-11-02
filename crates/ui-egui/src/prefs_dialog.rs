//! Edit → Preferences (⌘K): Illustrator-style category list + fields, rendered from the engine's
//! preference table ([`PREF_SPECS`]). The dialog edits a working copy in `Dialog::fields`
//! (one field per preference key, so agents can `ui.dialog.set` any of them). Apply keeps the
//! working copy open; OK applies it and closes, both through `prefs.set`. Also: persistence of engine prefs inside `UiState` and the per-frame
//! application of the UI-side preferences (brightness, canvas colour, UI scaling, render threads).

use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use vectorcraft_engine::Prefs;
use vectorcraft_engine::cmd::prefscmds::{GPU_PREFERENCES, PREF_CATEGORIES, PREF_GROUPS, PREF_SPECS, PrefKind};

use crate::state::Dialog;
use crate::theme::{self, Accent, Brightness, Scheme, Tokens};
use crate::{VectorcraftApp, widgets};

/// UI-only fields shown in the dialog (backed by `UiState`, not engine prefs).
pub const UI_FIELDS: &[(&str, &str, &str)] = &[
    ("__smartGuides", "Smart Guides", "Smart Guides (View → Smart Guides)"),
    ("__snapToGrid", "Guides & Grid", "Snap to Grid"),
    ("__taskBar", "General", "Enable Contextual Task Bar"),
];

/// The Preferences transaction kept beside the dialog. The setting values themselves stay in
/// `Dialog::fields`, the boundary agents read and `ui.dialog.set` writes; this holds what Apply
/// needs to know about them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreferencesDraft {
    /// The setting values at opening, or at the last Apply.
    baseline: Map<String, Value>,
    /// The fields the user edited since then, whatever their value now.
    touched: BTreeSet<String>,
    /// The document whose units the draft edits.
    document_uid: Option<u64>,
}

impl PreferencesDraft {
    fn new(app: &VectorcraftApp, baseline: Map<String, Value>) -> Self {
        Self { baseline, touched: BTreeSet::new(), document_uid: app.session.active().map(|doc| doc.uid) }
    }

    /// Record a user edit: Apply sends `key` even when it is back at the value Preferences opened
    /// with (a live change made meanwhile is overridden).
    fn touch(&mut self, key: &str) {
        if key != CATEGORY {
            self.touched.insert(key.to_string());
        }
    }
}

/// Navigation, not a setting: never applied, never compared.
const CATEGORY: &str = "__category";

/// Set one field of the open dialog. For Preferences it also counts as the user's edit, so UI
/// edits, Reset and agents' `ui.dialog.set` all record their edits through [`PreferencesDraft::touch`].
pub(crate) fn set_field(d: &mut Dialog, draft: &mut Option<PreferencesDraft>, key: String, value: Value) {
    if d.kind == "preferences"
        && let Some(draft) = draft
    {
        draft.touch(&key);
    }
    d.fields.insert(key, value);
}

/// A draft outlives no Preferences dialog (called after a dialog closes, and each frame).
pub(crate) fn settle(app: &mut VectorcraftApp) {
    if !app.ui.dialog.as_ref().is_some_and(|d| d.kind == "preferences") {
        app.ui.prefs_draft = None;
    }
}

fn current_fields(app: &VectorcraftApp) -> Map<String, Value> {
    let mut fields: Map<String, Value> = app.session.prefs.to_json().as_object().cloned().unwrap_or_default();
    fields.retain(|k, _| vectorcraft_engine::cmd::prefscmds::spec(k).is_some() || PREF_GROUPS.contains(&k.as_str()));
    if let Some(st) = app.session.active() {
        fields.insert("unitsGeneral".into(), json!(st.doc.units.key()));
    }
    fields.insert("__smartGuides".into(), json!(app.ui.view.smart_guides));
    fields.insert("__snapToGrid".into(), json!(app.ui.view.snap_to_grid));
    fields.insert("__taskBar".into(), json!(app.ui.task_bar));
    fields.insert(crate::varos::panel_stack::PREF_KEY.into(), json!(app.ui.varos_panel_stack.enabled));
    fields.insert(crate::varos::shell::PREF_KEY.into(), json!(app.ui.varos_shell.enabled));
    fields
}

pub fn open(app: &mut VectorcraftApp, category: Option<&str>) {
    let mut fields = current_fields(app);
    app.ui.prefs_draft = Some(PreferencesDraft::new(app, fields.clone()));
    let cat = category.and_then(|c| PREF_CATEGORIES.iter().find(|x| x.eq_ignore_ascii_case(c))).copied().unwrap_or(PREF_CATEGORIES[0]);
    fields.insert(CATEGORY.into(), json!(cat));
    app.ui.dialog = Some(Dialog { kind: "preferences".into(), fields });
}

/// Apply validates the edited settings before changing engine or UI state. A field the user
/// edited is applied even when it is back at its opening value; untouched draft fields never
/// overwrite settings changed in the modeless editor while Preferences was open. Cancel only
/// discards edits made after the last Apply. Units of a document that is no longer active are
/// skipped (with a status message); every other edit still applies.
pub fn apply(app: &mut VectorcraftApp) -> Result<Value, String> {
    let Some(mut d) = app.ui.dialog.clone() else { return Err("no dialog open".into()) };
    if d.kind != "preferences" {
        return Err("the open dialog is not Preferences".into());
    }
    // A dialog constructed without `open` has no draft and keeps the full-apply contract.
    let draft = app.ui.prefs_draft.as_ref();
    let mut changed: Map<String, Value> = d
        .fields
        .iter()
        .filter(|(key, _)| key.as_str() != CATEGORY)
        .filter(|(key, value)| draft.is_none_or(|draft| draft.touched.contains(*key) || draft.baseline.get(*key) != Some(*value)))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut skipped_units = false;
    if changed.contains_key("unitsGeneral")
        && let Some(draft) = draft
        && draft.document_uid != app.session.active().map(|doc| doc.uid)
    {
        changed.remove("unitsGeneral");
        skipped_units = true;
    }
    // UI-only fields are validated before prefs.set so a bad draft cannot partially apply.
    for key in ["__smartGuides", "__snapToGrid", "__taskBar", crate::varos::panel_stack::PREF_KEY, crate::varos::shell::PREF_KEY] {
        if changed.get(key).is_some_and(|value| !value.is_boolean()) {
            return Err(format!("{key}: expected a boolean"));
        }
    }
    let values: Map<String, Value> =
        changed.iter().filter(|(key, _)| !key.starts_with("__")).map(|(key, value)| (key.clone(), value.clone())).collect();
    let touch_ws = values.contains_key("touchWorkspace");
    if !values.is_empty() {
        app.run("prefs.set", json!({ "values": values }))?;
    }
    if touch_ws {
        crate::workspaces::sync_touch_workspace(app, app.touch_screen);
    }
    if let Some(on) = changed.get("__smartGuides").and_then(Value::as_bool) {
        app.ui.view.smart_guides = on;
    }
    if let Some(on) = changed.get("__snapToGrid").and_then(Value::as_bool) {
        app.ui.view.snap_to_grid = on;
    }
    if let Some(on) = changed.get("__taskBar").and_then(Value::as_bool) {
        app.ui.task_bar = on;
    }
    if let Some(on) = changed.get(crate::varos::panel_stack::PREF_KEY).and_then(Value::as_bool) {
        crate::varos::panel_stack::set_enabled(app, on);
    }
    if let Some(on) = changed.get(crate::varos::shell::PREF_KEY).and_then(Value::as_bool) {
        crate::varos::shell::set_enabled(app, on);
    }
    // Refresh untouched values from the live editor and establish the next Apply baseline.
    // Keep edited representations intact (for example an entered integer vs an engine float).
    let mut next = current_fields(app);
    for (key, value) in &changed {
        if next.contains_key(key) {
            next.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in &next {
        d.fields.insert(key.clone(), value.clone());
    }
    // A fresh edit record against the new baseline; the draft now edits the active document's units.
    app.ui.prefs_draft = Some(PreferencesDraft::new(app, next));
    app.ui.dialog = Some(d);
    app.ui.engine_prefs = app.session.prefs.to_json();
    app.prefs_changed = true;
    if skipped_units {
        let message = tl!("The active document changed, so its units were not changed. The other settings were applied.");
        app.status(message);
        return Ok(json!({ "skipped": ["unitsGeneral"], "message": message }));
    }
    Ok(Value::Null)
}

/// OK applies the same validated draft as Apply, then closes Preferences.
pub fn confirm(app: &mut VectorcraftApp) -> Result<Value, String> {
    let result = apply(app)?;
    app.ui.dialog = None;
    app.ui.prefs_draft = None;
    Ok(result)
}

/// Copy the engine prefs into `UiState` so the host persists them with the UI state.
pub fn snapshot(app: &mut VectorcraftApp) {
    app.ui.engine_prefs = app.session.prefs.to_json();
}

/// Restore engine prefs from `UiState` after the host loaded saved UI state.
pub fn restore(app: &mut VectorcraftApp) {
    let mut p: Prefs = serde_json::from_value(app.ui.engine_prefs.clone()).unwrap_or_default();
    if app.ui.engine_prefs.is_null() {
        // Older preference files: keep the saved brightness.
        p.ui_brightness = app.ui.brightness.id().into();
    }
    if matches!(p.ui_brightness.as_str(), "varos" | "varosWarm") {
        p.ui_brightness = "dark".into();
    } // Varos: migrate retired brightness ids.
    // Older preference files kept the interface language with the UI state; English was the
    // default there, so only another language is carried over (onto an unset preference).
    if let Some(code) = app.ui.legacy_language.take()
        && p.interface_language == "auto"
        && code != "en"
        && crate::i18n::Lang::from_code(&code).is_some()
    {
        p.interface_language = code;
    }
    // 0.5.0 saved its default, `powerSaving`, which Automatic replaced (#502): any value this
    // version doesn't offer reads as the default, as the desktop app reads it at startup.
    if !GPU_PREFERENCES.iter().any(|(v, _)| *v == p.gpu_preference) {
        p.gpu_preference = Prefs::default().gpu_preference;
    }
    crate::varos::middle_eastern_ui::default_on(app, &mut p); // Varos: Middle Eastern options on once
    app.session.apply_prefs(p);
}

#[derive(Clone, Copy, PartialEq)]
struct Applied {
    brightness: Brightness,
    accent: Accent,
    scheme: Scheme,
    /// User Interface › Canvas Color, unless it matches the interface brightness.
    canvas: Option<egui::Color32>,
    threads: i32,
    tool_tips: bool,
    scrub: bool,
    bare_points: bool,
}

/// User Interface › Canvas Color: the canvas around the artboards, or `None` to match the
/// interface brightness. The greys keep the white page's edge in view (#663).
fn canvas_color(key: &str) -> Option<egui::Color32> {
    match key {
        "white" => Some(egui::Color32::WHITE),
        "lightGray" => Some(egui::Color32::from_gray(0xc8)),
        "mediumGray" => Some(egui::Color32::from_gray(0x96)),
        "darkGray" => Some(egui::Color32::from_gray(0x5a)),
        _ => None,
    }
}

/// Per frame: push UI-side preferences into egui / the renderer when they change.
pub fn apply_runtime(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let p = &app.session.prefs;
    if let Some(b) = Brightness::parse(&p.ui_brightness)
        && b != app.ui.brightness
    {
        app.ui.brightness = b;
    }
    let want = Applied {
        brightness: app.ui.brightness,
        accent: Accent::parse(&p.ui_accent).unwrap_or_default(),
        scheme: Scheme::parse(&p.ui_color_scheme).unwrap_or_default(),
        canvas: canvas_color(&p.canvas_color),
        threads: p.render_threads,
        tool_tips: p.show_tool_tips,
        scrub: p.scrub_numeric_fields,
        bare_points: p.numbers_without_units_are_points,
    };
    let id = applied_id();
    let prev: Option<Applied> = ctx.data(|d| d.get_temp::<Option<Applied>>(id)).flatten();
    if prev != Some(want) {
        theme::apply_with(ctx, want.brightness, want.accent, want.scheme);
        // General › Show Tool Tips: off, no button or field shows its tool tip (they never come
        // due), whichever widget asks for one.
        let delay = if want.tool_tips { egui::style::Interaction::default().tooltip_delay } else { f32::INFINITY };
        ctx.global_style_mut(|s| s.interaction.tooltip_delay = delay);
        crate::scrub::set_enabled(ctx, want.scrub);
        crate::widgets::set_bare_numbers_are_points(ctx, want.bare_points);
        if let Some(c) = want.canvas {
            let mut t = Tokens::get(ctx);
            t.pasteboard = c;
            ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, t));
        }
        if prev.is_some_and(|pr| pr.threads != want.threads) {
            vectorcraft_render::set_default_threads(u16::try_from(want.threads).ok());
            app.canvas.renderer.threads = vectorcraft_render::default_threads();
            app.canvas.worker = None;
            app.canvas.worker_started = false;
        }
        app.canvas.key = None;
        ctx.data_mut(|d| d.insert_temp(id, Some(want)));
    }
    let z = app.session.prefs.ui_scaling.clamp(0.75, 2.0) as f32;
    if (ctx.zoom_factor() - z).abs() > 1e-3 {
        ctx.set_zoom_factor(z);
    }
    let touch = ctx.input(|i| i.has_touch_screen());
    if touch != app.touch_screen {
        app.touch_screen = touch;
        crate::workspaces::sync_touch_workspace(app, touch);
    }
}

/// Where [`apply_runtime`] keeps the preferences it last pushed into `ctx`.
fn applied_id() -> egui::Id {
    egui::Id::unique("dc-applied-prefs")
}

/// [`apply_runtime`] with nothing recorded as applied, so the theme (brightness, accent, color
/// scheme and canvas color) and the other UI-side preferences are pushed into `ctx` again.
pub(crate) fn reapply(app: &mut VectorcraftApp, ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Option<Applied>>(applied_id()));
    apply_runtime(app, ctx);
}

pub fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(mut d) = app.ui.dialog.clone() else { return };
    let drawn = d.fields.clone();
    let t = Tokens::get(ctx);
    let (mut ok, mut apply_now, mut cancel, mut reset) = (false, false, false, false);
    let mut open = true;
    let screen = ctx.content_rect();
    let body_height = (screen.height() - 160.0).clamp(80.0, 430.0);
    let cat = d.str("__category");
    let cat = PREF_CATEGORIES.iter().find(|c| **c == cat).copied().unwrap_or(PREF_CATEGORIES[0]);
    egui::Window::new(tl!("Preferences"))
        .id(egui::Id::unique("dialog-preferences"))
        .order(egui::Order::Foreground)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .movable(true)
        .default_pos(screen.center() - egui::vec2(400.0, (body_height + 110.0) * 0.5))
        .constrain_to(screen)
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(20)))
        .show(ctx, |ui| {
            ui.set_width(760.0);
            ui.horizontal_top(|ui| {
                // Category list.
                egui::Frame::NONE.fill(t.panel_darker).corner_radius(egui::CornerRadius::same(4)).inner_margin(egui::Margin::same(6)).show(
                    ui,
                    |ui| {
                        ui.set_width(196.0);
                        egui::ScrollArea::vertical().id_salt("prefs-categories").max_height(body_height).auto_shrink([false, false]).show(ui, |ui| {
                            // ScrollArea inherits the surrounding horizontal layout; categories
                            // need their own vertical column rather than one very wide row.
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                for c in PREF_CATEGORIES {
                                    let sel = *c == cat;
                                    let text = egui::RichText::new(tl!(*c)).size(12.5).color(if sel { t.text_strong } else { t.text });
                                    let b = egui::Button::selectable(sel, text).min_size(egui::vec2(184.0, 24.0));
                                    if ui.add(b).clicked() {
                                        d.fields.insert("__category".into(), json!(c));
                                    }
                                }
                            });
                        });
                    },
                );
                ui.add_space(14.0);
                // Fields.
                ui.vertical(|ui| {
                    ui.set_width(530.0);
                    ui.label(egui::RichText::new(tl!(cat)).font(theme::semibold(14.0)).color(t.text_strong));
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().id_salt(("prefs", cat)).max_height((body_height - 30.0).max(50.0)).auto_shrink([false, false]).show(
                        ui,
                        |ui| {
                            category_fields(ui, &mut d, cat);
                        },
                    );
                });
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, tl!("Reset Preferences"))
                    .on_hover_text(tl!("Restore every preference to its default (applied on Apply or OK)"))
                    .clicked()
                {
                    reset = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, tl!("OK")).clicked() {
                        ok = true;
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, tl!("Apply")).clicked() {
                        apply_now = true;
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, tl!("Cancel")).clicked() {
                        cancel = true;
                    }
                    ui.add_space(16.0);
                    let i = PREF_CATEGORIES.iter().position(|c| *c == cat).unwrap_or(0);
                    if ui.add_enabled(i + 1 < PREF_CATEGORIES.len(), egui::Button::new(tl!("Next"))).clicked() {
                        d.fields.insert("__category".into(), json!(PREF_CATEGORIES[i + 1]));
                    }
                    if ui.add_enabled(i > 0, egui::Button::new(tl!("Previous"))).clicked() {
                        d.fields.insert("__category".into(), json!(PREF_CATEGORIES[i - 1]));
                    }
                });
            });
        });
    if reset {
        for (k, v) in Prefs::default().to_json().as_object().cloned().unwrap_or_default() {
            set_field(&mut d, &mut app.ui.prefs_draft, k, v);
        }
    }
    // Every field a widget changed this frame counts as the user's edit until the next Apply.
    if let Some(draft) = app.ui.prefs_draft.as_mut() {
        for (key, value) in &d.fields {
            if drawn.get(key) != Some(value) {
                draft.touch(key);
            }
        }
    }
    app.ui.dialog = Some(d);
    if cancel || !open {
        crate::dialogs::cancel(app);
    } else if ok {
        if let Err(e) = confirm(app) {
            app.status(e);
        }
    } else if apply_now && let Err(e) = apply(app) {
        app.status(e);
    }
}

fn category_fields(ui: &mut egui::Ui, d: &mut Dialog, cat: &str) {
    let t = Tokens::get(ui.ctx());
    let mut section = "";
    let mut first = true;
    let specs: Vec<_> = PREF_SPECS.iter().filter(|s| s.category == cat).collect();
    for (key, c, label) in UI_FIELDS {
        if *c == cat && cat != "Smart Guides" {
            bool_row(ui, d, key, label);
        }
    }
    if cat == "Smart Guides" {
        bool_row(ui, d, "__smartGuides", UI_FIELDS[0].2);
        ui.add_space(4.0);
    }
    for sp in specs {
        if sp.section != section {
            section = sp.section;
            if !section.is_empty() {
                if !first {
                    ui.add_space(8.0);
                }
                ui.label(egui::RichText::new(tl!(section)).font(theme::semibold(12.5)).color(t.text_strong));
                ui.add_space(2.0);
            }
        }
        first = false;
        let v = d.fields.get(sp.key).cloned().unwrap_or(Value::Null);
        match sp.kind {
            // Units › Numbers Without Units Are Points only tells points from picas: dimmed
            // unless a unit is Picas.
            PrefKind::Bool if sp.key == "numbersWithoutUnitsArePoints" => {
                ui.add_enabled_ui(picas_in_use(d), |ui| bool_row(ui, d, sp.key, sp.label));
            }
            // Performance › Animated Zoom needs GPU Performance: dimmed while it is off.
            PrefKind::Bool if sp.key == "animatedZoom" => {
                ui.add_enabled_ui(d.bool("gpuPerformance"), |ui| bool_row(ui, d, sp.key, sp.label));
            }
            // Windows and Linux only: macOS always has the system's title bar, the web has none.
            PrefKind::Bool if sp.key == "systemTitleBar" => {
                if cfg!(all(not(target_arch = "wasm32"), not(target_os = "macos"))) {
                    bool_row(ui, d, sp.key, sp.label);
                    // The window's decorations are chosen once, when the app starts.
                    ui.label(egui::RichText::new(tl!("Applies the next time VectorCraft starts.")).color(t.text_dim).size(11.0));
                }
            }
            // Varos: Panel cards (Varos) under Auto-Collapse Iconic Panels, which it sits beside.
            PrefKind::Bool if sp.key == "autoCollapseIconPanels" => {
                bool_row(ui, d, sp.key, sp.label);
                bool_row(ui, d, crate::varos::panel_stack::PREF_KEY, crate::varos::panel_stack::PREF_LABEL);
                bool_row(ui, d, crate::varos::shell::PREF_KEY, crate::varos::shell::PREF_LABEL);
            }
            PrefKind::Bool => bool_row(ui, d, sp.key, sp.label),
            PrefKind::Num { min, max, unit } => {
                labeled(ui, sp.label, |ui| {
                    let mut x = v.as_f64().unwrap_or(min);
                    let new = if sp.key == "uiScaling" {
                        widgets::slider(ui, egui::Slider::new(&mut x, min..=max).step_by(0.05).text(tl!("Smaller ↔ Larger"))).changed().then_some(x)
                    } else {
                        widgets::range_field(ui, sp.key, x, min..=max, &format!(" {unit}"), 3, 110.0)
                    };
                    if let Some(x) = new {
                        d.fields.insert(sp.key.into(), json!(x));
                    }
                });
            }
            PrefKind::Length { min, max, measure } => {
                // In the dialog's own Units choice, so a change there shows right away.
                let unit = vectorcraft_doc::Unit::named(&d.str(measure.pref_key())).unwrap_or_default();
                let x = d.f64(sp.key, min);
                labeled(ui, sp.label, |ui| {
                    if let Some(x) = widgets::num_field(ui, sp.key, Some(x), unit, 110.0) {
                        d.fields.insert(sp.key.into(), json!(x.clamp(min, max)));
                    }
                });
            }
            PrefKind::Int { min, max } => {
                labeled(ui, sp.label, |ui| {
                    let mut x = v.as_i64().unwrap_or(min);
                    let new = if sp.key == "anchorSize" {
                        widgets::slider(ui, egui::Slider::new(&mut x, min..=max).show_value(false).text(tl!("Size"))).changed().then_some(x)
                    } else {
                        widgets::range_field(ui, sp.key, x as f64, min as f64..=max as f64, "", 0, 110.0).map(|x| x as i64)
                    };
                    if let Some(x) = new {
                        d.fields.insert(sp.key.into(), json!(x));
                    }
                });
            }
            // Linux only: the other platforms have a single display server.
            PrefKind::Choice(_) if sp.key == "linuxDisplayServer" && !cfg!(target_os = "linux") => {}
            // The Classic colour scheme has its own accent: dimmed while it is chosen.
            PrefKind::Choice(opts) if sp.key == "uiAccent" => {
                let modern = Scheme::parse(&d.str("uiColorScheme")).unwrap_or_default() == Scheme::Modern;
                ui.add_enabled_ui(modern, |ui| choice_row(ui, d, sp.key, sp.label, opts, &v));
            }
            PrefKind::Choice(opts) => {
                choice_row(ui, d, sp.key, sp.label, opts, &v);
                if sp.key == "gpuPreference" || sp.key == "linuxDisplayServer" {
                    // The window's graphics device and display server are chosen once, when the app starts.
                    ui.label(egui::RichText::new(tl!("Applies the next time VectorCraft starts.")).color(t.text_dim).size(11.0));
                }
                if sp.key == "linuxDisplayServer" {
                    // Many Wayland desktops give a drawing tablet's pen to the window only through XWayland (#2028).
                    ui.label(
                        egui::RichText::new(tl!("If a drawing tablet's pen doesn't draw, choose an X11 (XWayland) option."))
                            .color(t.text_dim)
                            .size(11.0),
                    );
                }
            }
            PrefKind::Color => {
                labeled(ui, sp.label, |ui| {
                    let hex = v.as_str().unwrap_or("#000000");
                    let c = vectorcraft_color::Color::from_hex(hex).map(|c| c.to_rgba8(1.0)).unwrap_or([0, 0, 0, 255]);
                    let mut rgb = [c[0], c[1], c[2]];
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        d.fields.insert(sp.key.into(), json!(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])));
                    }
                    ui.label(egui::RichText::new(hex).color(t.text_dim).size(11.0));
                });
            }
            PrefKind::Text if sp.key == "interfaceLanguage" => {
                // `auto` plus every registered language, by its own name.
                labeled(ui, sp.label, |ui| {
                    let cur = v.as_str().unwrap_or("auto");
                    let codes: Vec<&str> = std::iter::once("auto").chain(crate::i18n::Lang::for_picker().into_iter().map(|l| l.code())).collect();
                    let labels: Vec<&str> =
                        std::iter::once(tl!("Auto")).chain(crate::i18n::Lang::for_picker().into_iter().map(|l| l.name())).collect();
                    let cur_label = codes.iter().position(|c| c.eq_ignore_ascii_case(cur)).and_then(|i| labels.get(i).copied()).unwrap_or(cur);
                    if let Some(i) = widgets::dropdown(ui, sp.key, cur_label, &labels, 260.0)
                        && let Some(code) = codes.get(i)
                    {
                        d.fields.insert(sp.key.into(), json!(code));
                    }
                });
            }
            PrefKind::Text => {
                labeled(ui, sp.label, |ui| {
                    let mut s = v.as_str().unwrap_or("").to_string();
                    if ui.add(egui::TextEdit::singleline(&mut s).desired_width(260.0)).changed() {
                        d.fields.insert(sp.key.into(), json!(s));
                    }
                });
            }
        }
    }
}

/// Is one of the dialog's Units (General, Stroke, Type, East Asian Type) Picas?
fn picas_in_use(d: &Dialog) -> bool {
    ["unitsGeneral", "unitsStroke", "unitsType", "unitsAsianType"].iter().any(|k| d.str(k) == vectorcraft_doc::Unit::Picas.key())
}

fn choice_row(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str, opts: &[(&str, &str)], v: &Value) {
    labeled(ui, label, |ui| {
        let cur = v.as_str().unwrap_or("");
        let cur_label = opts.iter().find(|o| o.0 == cur).map(|o| o.1).unwrap_or(cur);
        let labels: Vec<&str> = opts.iter().map(|o| o.1).collect();
        if let Some(i) = widgets::dropdown(ui, key, cur_label, &labels, 260.0)
            && let Some(o) = opts.get(i)
        {
            d.fields.insert(key.into(), json!(o.0));
        }
    });
}

fn bool_row(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    // The app's outlined checkbox (it translates `label`): egui's own has no border at rest.
    let on = d.bool(key);
    if widgets::check(ui, label, on, ui.is_enabled()) {
        d.fields.insert(key.into(), json!(!on));
    }
}

fn labeled(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        let label = ui
            .allocate_ui_with_layout(egui::vec2(210.0, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(210.0);
                ui.add(egui::Label::new(egui::RichText::new(format!("{}{}", tl!(label), tl!(":"))).color(t.text)).truncate())
            })
            .inner;
        ui.label_inputs_by(label.id, add);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use vectorcraft_engine::Session;

    #[test]
    fn geometric_snapping_is_a_saved_smart_guides_preference() {
        let mut a = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        open(&mut a, Some("Smart Guides"));
        let text = crate::tests_labels::painted_text(&mut a, |a, ui| show(a, ui.ctx()));
        assert!(text.contains("Geometric Guides") && text.contains("Snap to Perpendicular") && text.contains("Snap to Tangent"), "{text}");
        a.ui.dialog.as_mut().unwrap().fields.insert("snapPerpendicular".into(), json!(false));
        confirm(&mut a).unwrap();
        assert!(!a.session.prefs.snap_perpendicular && a.session.prefs.snap_tangent);
        let saved: Prefs = serde_json::from_value(a.ui.engine_prefs.clone()).unwrap();
        assert!(!saved.snap_perpendicular && saved.snap_tangent);
        assert!(a.run("view.snapping", json!({})).is_err());
    }

    /// A resting checkbox must have a visible outline in both states and every brightness.
    #[test]
    fn preference_checkboxes_draw_outlines_without_hover() {
        for brightness in Brightness::ALL {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            theme::apply(&ctx, brightness);
            for checked in [false, true] {
                let mut d = Dialog::new("preferences", json!({"showToolTips": checked}));
                let raw =
                    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))), ..Default::default() };
                let mut out = ctx.run_ui(raw, |ui| bool_row(ui, &mut d, "showToolTips", "Show Tool Tips"));
                out.textures_delta.clear();
                let outlined = out.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Rect(r) => r.stroke.width > 0.0 && r.rect.width() <= 24.0 && r.rect.height() <= 24.0,
                    _ => false,
                });
                assert!(outlined, "{brightness:?}, checked={checked}: no checkbox outline");
            }
        }
    }

    /// User Interface › Canvas Color (#663): White and the three greys paint the canvas around the
    /// artboards; Match User Interface Brightness keeps the theme's.
    #[test]
    fn canvas_color_choices_paint_the_pasteboard() {
        let mut app = VectorcraftApp::new(Session::new(), Services::default());
        let ctx = egui::Context::default();
        apply_runtime(&mut app, &ctx);
        let themed = Tokens::get(&ctx).pasteboard;
        for (key, want) in [
            ("white", egui::Color32::WHITE),
            ("lightGray", egui::Color32::from_gray(0xc8)),
            ("mediumGray", egui::Color32::from_gray(0x96)),
            ("darkGray", egui::Color32::from_gray(0x5a)),
            ("matchUi", themed),
        ] {
            app.run("prefs.set", serde_json::json!({"key": "canvasColor", "value": key})).unwrap();
            apply_runtime(&mut app, &ctx);
            assert_eq!(Tokens::get(&ctx).pasteboard, want, "{key}");
        }
        assert!(app.run("prefs.set", serde_json::json!({"key": "canvasColor", "value": "pink"})).is_err());
    }

    /// #4837: the saved Color Scheme, Accent and Canvas Color are the ones the first frame shows, not
    /// the defaults the first UI pass installs.
    #[test]
    fn saved_scheme_accent_and_canvas_color_show_from_the_first_frame() {
        let mut app = app();
        for (key, value) in [("uiColorScheme", "classic"), ("uiAccent", "pink"), ("canvasColor", "white")] {
            app.run("prefs.set", json!({"key": key, "value": value})).unwrap();
        }
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.logic(ui.ctx()));
            out.textures_delta.clear();
            let (t, classic) = (Tokens::get(&ctx), Tokens::for_scheme(Scheme::Classic, app.ui.brightness, Accent::Pink));
            assert_eq!((t.accent, t.app_bar, t.panel), (classic.accent, classic.app_bar, classic.panel));
            assert_eq!(t.pasteboard, egui::Color32::WHITE);
            // Varos deliberately uses neutral links, even with a custom upstream accent.
            assert_eq!(ctx.global_style().visuals.hyperlink_color, classic.text);
        }
    }

    fn app() -> VectorcraftApp {
        VectorcraftApp::new(Session::new(), Services::default())
    }

    #[test]
    fn apply_keeps_the_draft_and_category_then_cancel_preserves_applied_values() {
        let mut a = app();
        open(&mut a, Some("Smart Guides"));
        let d = a.ui.dialog.as_mut().unwrap();
        d.fields.insert("keyboardIncrement".into(), json!(4));
        d.fields.insert("__smartGuides".into(), json!(false));
        let draft = d.fields.clone();
        apply(&mut a).unwrap();
        let after = &a.ui.dialog.as_ref().unwrap().fields;
        for (key, value) in &draft {
            assert_eq!(after.get(key), Some(value), "draft field {key}");
        }
        assert_eq!(a.session.prefs.keyboard_increment, 4.0);
        assert!(!a.ui.view.smart_guides);
        assert_eq!(a.ui.engine_prefs["keyboardIncrement"], json!(4.0));
        assert!(a.prefs_changed);
        let d = a.ui.dialog.as_mut().unwrap();
        d.fields.insert("keyboardIncrement".into(), json!(9));
        d.fields.insert("__smartGuides".into(), json!(true));
        crate::dialogs::cancel(&mut a);
        assert!(a.ui.dialog.is_none());
        assert_eq!(a.session.prefs.keyboard_increment, 4.0);
        assert!(!a.ui.view.smart_guides);
        assert_eq!(a.ui.engine_prefs["keyboardIncrement"], json!(4.0));
    }

    #[test]
    fn modeless_apply_merges_only_edited_fields_with_live_editor_preferences() {
        let mut a = app();
        open(&mut a, Some("General"));
        a.ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(4));
        let smart = !a.ui.view.smart_guides;
        let grid = !a.ui.view.snap_to_grid;
        a.run("prefs.set", json!({"key": "gridlineEvery", "value": 73})).unwrap();
        a.ui.view.smart_guides = smart;
        a.ui.view.snap_to_grid = grid;
        apply(&mut a).unwrap();
        assert_eq!(a.session.prefs.keyboard_increment, 4.0);
        assert_eq!(a.session.prefs.gridline_every, 73.0);
        assert_eq!(a.ui.view.smart_guides, smart);
        assert_eq!(a.ui.view.snap_to_grid, grid);
        let d = a.ui.dialog.as_ref().unwrap();
        assert_eq!(d.fields["gridlineEvery"], json!(73.0));
        assert_eq!(d.fields["__smartGuides"], json!(smart));
        assert_eq!(d.fields["__snapToGrid"], json!(grid));
        // The next Apply also leaves an externally changed, untouched field alone.
        a.run("prefs.set", json!({"key": "keyboardIncrement", "value": 7})).unwrap();
        apply(&mut a).unwrap();
        assert_eq!(a.session.prefs.keyboard_increment, 7.0);
        assert_eq!(a.ui.dialog.as_ref().unwrap().fields["keyboardIncrement"], json!(7.0));
    }

    #[test]
    fn modeless_units_edit_for_a_document_no_longer_active_is_skipped_and_the_rest_applies() {
        let mut a = app();
        a.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        open(&mut a, Some("Units"));
        let original_units = a.ui.dialog.as_ref().unwrap().fields["unitsGeneral"].clone();
        let changed_units = if original_units == "millimeters" { "points" } else { "millimeters" };
        a.ui.dialog.as_mut().unwrap().fields.insert("unitsGeneral".into(), json!(changed_units));
        a.ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(4));
        a.run("file.new", json!({"width": 300, "height": 200})).unwrap();
        let units: Vec<_> = a.session.documents().iter().map(|doc| doc.doc.units).collect();
        let active = a.session.active_index();
        let result = apply(&mut a).unwrap();
        assert_eq!(result["skipped"], json!(["unitsGeneral"]), "the units edit is reported as skipped");
        assert!(a.ui.status.contains("active document changed"), "{}", a.ui.status);
        assert_eq!(a.session.prefs.keyboard_increment, 4.0, "unrelated edits apply");
        assert_eq!(a.session.active_index(), active);
        assert_eq!(a.session.documents().iter().map(|doc| doc.doc.units).collect::<Vec<_>>(), units, "no document's units change");
        let d = a.ui.dialog.as_ref().unwrap();
        assert_eq!(d.str("__category"), "Units");
        assert_eq!(a.ui.prefs_draft.as_ref().unwrap().document_uid, a.session.active().map(|doc| doc.uid), "the draft now edits the active document");
        assert_eq!(d.fields["unitsGeneral"], json!(a.session.active().unwrap().doc.units.key()), "the units field shows the active document's");
        // A units edit made now applies to the active document.
        a.ui.dialog.as_mut().unwrap().fields.insert("unitsGeneral".into(), json!(changed_units));
        assert!(apply(&mut a).unwrap().is_null());
        assert_eq!(a.session.active().unwrap().doc.units.key(), changed_units);
    }

    /// A field set back to the value Preferences opened with still applies when the user touched
    /// it, over a change made elsewhere meanwhile; an untouched field still keeps that change.
    #[test]
    fn a_touched_field_back_at_its_opening_value_overrides_a_live_change() {
        let mut a = app();
        open(&mut a, Some("General"));
        let opening = a.session.prefs.keyboard_increment;
        a.run("prefs.set", json!({"key": "keyboardIncrement", "value": opening + 6.0})).unwrap();
        a.run("prefs.set", json!({"key": "gridlineEvery", "value": 73})).unwrap();
        let d = a.ui.dialog.as_mut().unwrap();
        d.fields.insert("keyboardIncrement".into(), json!(opening + 4.0));
        d.fields.insert("keyboardIncrement".into(), json!(opening));
        set_field(d, &mut a.ui.prefs_draft, "keyboardIncrement".into(), json!(opening));
        apply(&mut a).unwrap();
        assert_eq!(a.session.prefs.keyboard_increment, opening, "the touched field wins");
        assert_eq!(a.session.prefs.gridline_every, 73.0, "the untouched field keeps the live change");
        assert!(a.ui.prefs_draft.as_ref().unwrap().touched.is_empty(), "Apply starts a fresh edit record");
    }

    /// Agents see only setting values and the category; the transaction stays typed beside them.
    #[test]
    fn the_dialog_fields_carry_no_transaction_metadata_and_agent_edits_count_as_touched() {
        let mut a = app();
        open(&mut a, Some("General"));
        let d = a.ui.dialog.as_mut().unwrap();
        let opening = d.fields["keyboardIncrement"].clone();
        set_field(d, &mut a.ui.prefs_draft, "keyboardIncrement".into(), opening);
        let fields = &a.ui.dialog.as_ref().unwrap().fields;
        for meta in ["__baseline", "__touched", "__documentUid"] {
            assert!(!fields.contains_key(meta), "{meta} stays out of the agent-visible fields");
        }
        assert!(a.ui.prefs_draft.as_ref().unwrap().touched.contains("keyboardIncrement"));
        crate::dialogs::cancel(&mut a);
        assert!(a.ui.prefs_draft.is_none(), "no draft outlives its dialog");
    }

    #[test]
    fn invalid_apply_leaves_engine_ui_and_saved_preferences_unchanged() {
        let mut a = app();
        open(&mut a, Some("User Interface"));
        let before = a.session.prefs.clone();
        let saved = a.ui.engine_prefs.clone();
        let smart = a.ui.view.smart_guides;
        let changed = a.prefs_changed;
        let d = a.ui.dialog.as_mut().unwrap();
        d.fields.insert("keyboardIncrement".into(), json!(4));
        d.fields.insert("anchorSize".into(), json!(42));
        d.fields.insert("__smartGuides".into(), json!(!smart));
        let draft = d.fields.clone();
        assert!(apply(&mut a).is_err());
        assert_eq!(a.session.prefs, before);
        assert_eq!(a.ui.engine_prefs, saved);
        assert_eq!(a.ui.view.smart_guides, smart);
        assert_eq!(a.prefs_changed, changed);
        assert_eq!(a.ui.dialog.as_ref().unwrap().fields, draft);
    }

    #[test]
    fn preferences_apply_button_stays_open_and_outside_clicks_are_modeless() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut a = app();
        open(&mut a, Some("General"));
        a.ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(6));
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).with_context_setup(crate::theme::install_fonts).build_ui_state(
            |ui, a: &mut VectorcraftApp| {
                if ui.button("Outside Preferences").clicked() {
                    a.status("outside click reached editor");
                }
                if a.ui.dialog.is_some() {
                    show(a, ui.ctx());
                }
            },
            a,
        );
        h.run_steps(3);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Apply").click();
        h.run_steps(3);
        assert_eq!(h.state().session.prefs.keyboard_increment, 6.0);
        assert_eq!(h.state().ui.dialog.as_ref().unwrap().str("__category"), "General");
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Outside Preferences").click();
        h.run_steps(3);
        assert_eq!(h.state().ui.status, "outside click reached editor");
        assert!(h.state().ui.dialog.is_some());
        h.state_mut().ui.dialog.as_mut().unwrap().fields.insert("keyboardIncrement".into(), json!(8));
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Cancel").click();
        h.run_steps(3);
        assert!(h.state().ui.dialog.is_none());
        assert_eq!(h.state().session.prefs.keyboard_increment, 6.0);
    }

    #[test]
    fn dialog_ok_applies_prefs_and_ui_fields() {
        let mut a = app();
        open(&mut a, Some("general"));
        let d = a.ui.dialog.as_mut().unwrap();
        assert_eq!(d.str("__category"), "General");
        d.fields.insert("keyboardIncrement".into(), json!(4));
        d.fields.insert("uiBrightness".into(), json!("light"));
        d.fields.insert("__smartGuides".into(), json!(false));
        confirm(&mut a).unwrap();
        assert!(a.ui.dialog.is_none());
        assert_eq!(a.session.prefs.keyboard_increment, 4.0);
        assert_eq!(a.session.prefs.ui_brightness, "light");
        assert!(!a.ui.view.smart_guides);
        assert_eq!(a.ui.engine_prefs["keyboardIncrement"], json!(4.0));
    }

    /// General › Show Tool Tips: off, a button held under the pointer shows no tool tip; back on,
    /// it does again (#394).
    #[test]
    fn show_tool_tips_off_hides_tool_tips() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut a = app();
        let ctx = egui::Context::default();
        let mut time = 0.0;
        // Come from elsewhere and hover the button for two seconds: did its tool tip show?
        let mut hover = |a: &mut VectorcraftApp| {
            let mut shown = vec![];
            for i in 0..20 {
                time += 0.1;
                let events = match i {
                    0 => vec![egui::Event::PointerGone],
                    1 => vec![egui::Event::PointerMoved(egui::pos2(20.0, 12.0))],
                    _ => vec![],
                };
                let raw = egui::RawInput {
                    events,
                    time: Some(time),
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(raw, |ui| {
                    apply_runtime(a, ui.ctx());
                    ui.button("Button").on_hover_text("The tool tip");
                });
                out.textures_delta.clear();
                out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
            }
            shown.iter().any(|t| t == "The tool tip")
        };
        assert!(hover(&mut a), "on by default");
        a.run("prefs.set", json!({"key": "showToolTips", "value": false})).unwrap();
        assert!(!hover(&mut a), "off: no tool tip");
        a.run("prefs.set", json!({"key": "showToolTips", "value": true})).unwrap();
        assert!(hover(&mut a), "on again");
    }

    #[test]
    fn dialog_ok_with_invalid_value_keeps_dialog_and_prefs() {
        let mut a = app();
        let before = a.session.prefs.clone(); // Varos: compare against the UI's actual default appearance.
        open(&mut a, None);
        a.ui.dialog.as_mut().unwrap().fields.insert("anchorSize".into(), json!(42));
        assert!(confirm(&mut a).is_err());
        assert!(a.ui.dialog.is_some());
        assert_eq!(a.session.prefs, before);
    }

    #[test]
    fn prefs_persist_through_ui_state() {
        let mut a = app();
        a.run("prefs.set", json!({"key": "gridlineEvery", "value": 50})).unwrap();
        snapshot(&mut a);
        let saved = serde_json::to_vec(&a.ui).unwrap();
        let mut b = app();
        b.ui = serde_json::from_slice::<crate::UiState>(&saved).unwrap().sanitized();
        restore(&mut b);
        assert_eq!(b.session.prefs.gridline_every, 50.0);
    }

    #[test]
    fn old_pref_files_keep_their_brightness() {
        let mut a = app();
        a.ui.brightness = Brightness::Light;
        a.ui.engine_prefs = Value::Null;
        restore(&mut a);
        assert_eq!(a.session.prefs.ui_brightness, "light");
    }

    /// Performance › Graphics Processor (#306, #502): shown with its restart note, applied by OK
    /// and saved with the UI state under the key the desktop app reads before the window opens.
    #[test]
    fn graphics_processor_preference_shows_and_persists() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut a = app();
        open(&mut a, Some("Performance"));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // The window sizes itself on the first frame and draws on the second.
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut shown = vec![];
        out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
        assert!(shown.iter().any(|t| t.starts_with("Graphics Processor")), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Automatic"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Applies the next time Varos starts."), "{shown:?}");
        a.ui.dialog.as_mut().unwrap().fields.insert("gpuPreference".into(), json!("highPerformance"));
        assert!(!a.take_prefs_changed(), "nothing to write before OK");
        confirm(&mut a).unwrap();
        assert!(a.take_prefs_changed(), "OK asks the host to write the preferences file");
        assert!(!a.take_prefs_changed());
        assert_eq!(a.session.prefs.gpu_preference, "highPerformance");
        let saved: Value = serde_json::from_slice(&serde_json::to_vec(&a.ui).unwrap()).unwrap();
        assert_eq!(saved["engine_prefs"]["gpuPreference"], json!("highPerformance"));
    }

    /// User Interface › System Title Bar: shown with its next-launch note, applied by OK and
    /// saved with the UI state under the key the desktop app reads before the window opens.
    #[test]
    fn system_title_bar_preference_shows_and_persists() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut a = app();
        open(&mut a, Some("User Interface"));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // The window sizes itself on the first frame and draws on the second.
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut shown = vec![];
        out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
        let supported = cfg!(all(not(target_arch = "wasm32"), not(target_os = "macos")));
        assert_eq!(shown.iter().any(|t| t == "System Title Bar"), supported, "{shown:?}");
        assert_eq!(shown.iter().any(|t| t == "Applies the next time Varos starts."), supported, "{shown:?}");
        a.ui.dialog.as_mut().unwrap().fields.insert("systemTitleBar".into(), json!(true));
        confirm(&mut a).unwrap();
        assert!(a.session.prefs.system_title_bar);
        let saved: Value = serde_json::from_slice(&serde_json::to_vec(&a.ui).unwrap()).unwrap();
        assert_eq!(saved["engine_prefs"]["systemTitleBar"], json!(true));
    }

    /// Performance › Linux Display Server (#2028): on Linux it shows the drawing-tablet note, and
    /// X11 (XWayland) When a Pen Is Attached is applied by OK and saved under the key the desktop
    /// app reads before the window opens.
    #[test]
    fn linux_display_server_offers_the_pen_choice() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut a = app();
        open(&mut a, Some("Performance"));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // The window sizes itself on the first frame and draws on the second.
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut shown = vec![];
        out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
        let linux = cfg!(target_os = "linux");
        assert_eq!(shown.iter().any(|t| t.starts_with("Linux Display Server")), linux, "{shown:?}");
        let note = "If a drawing tablet's pen doesn't draw, choose an X11 (XWayland) option.";
        assert_eq!(shown.iter().any(|t| t == note), linux, "{shown:?}");
        a.ui.dialog.as_mut().unwrap().fields.insert("linuxDisplayServer".into(), json!("x11WithPen"));
        confirm(&mut a).unwrap();
        assert_eq!(a.session.prefs.linux_display_server, "x11WithPen");
        let saved: Value = serde_json::from_slice(&serde_json::to_vec(&a.ui).unwrap()).unwrap();
        assert_eq!(saved["engine_prefs"]["linuxDisplayServer"], json!("x11WithPen"));
    }

    /// #1866: the category shown is highlighted in the list while the pointer is elsewhere.
    #[test]
    fn the_category_shown_is_highlighted() {
        fn rects(s: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
            match s {
                egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| rects(s, out)),
                _ => {}
            }
        }
        let mut a = app();
        open(&mut a, Some("Units"));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // The window sizes itself on the first frame and draws on the second.
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut a, ui.ctx()));
        out.textures_delta.clear();
        let mut shown = vec![];
        out.shapes.iter().for_each(|c| rects(&c.shape, &mut shown));
        // The list's rows, top to bottom: only the fourth (Units) is filled.
        let rows: Vec<_> = shown.iter().filter(|(r, _)| r.width() == 184.0 && r.height() < 30.0).collect();
        assert_eq!(rows.len(), PREF_CATEGORIES.len(), "{shown:?}");
        let filled: Vec<usize> = rows.iter().enumerate().filter(|(_, (_, c))| c.a() > 0).map(|(i, _)| i).collect();
        assert_eq!(filled, vec![3], "{rows:?}");
    }

    /// Units › Numbers Without Units Are Points (#394): the preference reaches the fields when it
    /// changes, and its checkbox is dimmed unless a unit is Picas.
    #[test]
    fn numbers_without_units_are_points_reaches_the_fields_and_dims_without_picas() {
        let mut a = app();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let frame = |a: &mut VectorcraftApp| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                apply_runtime(a, ui.ctx());
                show(a, ui.ctx());
            });
            out.textures_delta.clear();
        };
        let read = |ctx: &egui::Context| widgets::typed_unit(ctx, vectorcraft_doc::Unit::Picas);
        frame(&mut a);
        assert_eq!(read(&ctx), vectorcraft_doc::Unit::Points, "on by default");
        a.run("prefs.set", json!({"key": "numbersWithoutUnitsArePoints", "value": false})).unwrap();
        frame(&mut a);
        assert_eq!(read(&ctx), vectorcraft_doc::Unit::Picas, "off");
        // The checkbox: dimmed with every unit in points, enabled once a unit is Picas.
        open(&mut a, Some("Units"));
        let d = a.ui.dialog.as_mut().unwrap();
        assert!(!picas_in_use(d), "dimmed in points");
        d.fields.insert("unitsStroke".into(), json!("picas"));
        assert!(picas_in_use(d), "enabled with a unit in picas");
    }

    /// 0.5.0 saved `powerSaving` for everyone (its default): it reads as Automatic, while the
    /// choices this version offers are kept (#502).
    #[test]
    fn a_graphics_processor_this_version_does_not_offer_reads_as_automatic() {
        for (saved, read) in [("powerSaving", "automatic"), ("turbo", "automatic"), ("lowPower", "lowPower"), ("highPerformance", "highPerformance")]
        {
            let mut a = app();
            a.ui.engine_prefs = json!({"gpuPreference": saved});
            restore(&mut a);
            assert_eq!(a.session.prefs.gpu_preference, read, "{saved}");
        }
    }

    #[test]
    fn every_category_renders_fields() {
        for c in PREF_CATEGORIES {
            assert!(PREF_SPECS.iter().any(|s| s.category == *c));
        }
    }

    /// Preferences › Type › Show Font Names in English (#394): OK on the dialog stores the flag the
    /// font menus read (Character panel, Type → Font).
    #[test]
    fn show_font_names_in_english_dialog_ok_stores_the_flag() {
        let mut a = app();
        assert!(a.session.prefs.font_names_in_english);
        open(&mut a, Some("Type"));
        assert_eq!(a.ui.dialog.as_ref().unwrap().str("__category"), "Type");
        a.ui.dialog.as_mut().unwrap().fields.insert("fontNamesInEnglish".into(), json!(false));
        confirm(&mut a).unwrap();
        assert!(a.ui.dialog.is_none());
        assert!(!a.session.prefs.font_names_in_english);
        assert!(!crate::font_menu::MenuLook::of(&a).english_names);
    }

    /// Preferences › Hyphenation › Exceptions (#394): OK on the dialog pushes the list into the
    /// hyphenator the same way `prefs.set` does (no need to drive the live Preferences window).
    #[test]
    fn hyphenation_exceptions_dialog_ok_reaches_the_hyphenator() {
        struct Clear;
        impl Drop for Clear {
            fn drop(&mut self) {
                vectorcraft_text::set_hyphenation_exceptions("");
            }
        }
        let _clear = Clear;
        let mut a = app();
        open(&mut a, Some("Hyphenation"));
        let d = a.ui.dialog.as_mut().unwrap();
        assert_eq!(d.str("__category"), "Hyphenation");
        d.fields.insert("hyphenationExceptions".into(), json!("typography, hap-pen"));
        confirm(&mut a).unwrap();
        assert!(a.ui.dialog.is_none());
        assert_eq!(a.session.prefs.hyphenation_exceptions, "typography, hap-pen");
        assert_eq!(vectorcraft_text::hyphenation_exceptions(), "typography, hap-pen");
        assert!(vectorcraft_text::hyphen::hyphen_points("typography").is_empty());
        assert_eq!(vectorcraft_text::hyphen::hyphen_points("happen"), vec![3]);
    }
}
