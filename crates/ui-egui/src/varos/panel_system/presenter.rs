//! The one owner of "which presenter draws the editor chrome". Rendering, commands, workspace
//! queries, menu checks and keyboard capture ask here; nothing else reads the raw panel state to
//! decide whether the unified workspace is active.
use super::State;
use crate::{VectorcraftApp, state::UiState};

/// What the unified panel workspace presents this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presenter {
    /// Another presenter (the Varos shell boxes or the classic dock) owns the editor chrome.
    Inactive,
    /// The workspace owns the chrome, but Home covers the whole central surface.
    Home,
    /// The workspace draws the editor chrome.
    Workspace,
}

/// The workspace state while it owns the editor chrome; `None` while another presenter does.
/// Turning the Varos shell off in Preferences returns to the classic dock without discarding the
/// saved layout, so the state can exist while it is not presented.
pub(crate) fn state(ui: &UiState) -> Option<&State> {
    if ui.varos_shell.enabled { ui.varos_panel_review.as_ref() } else { None }
}

/// [`state`], mutable.
pub(crate) fn state_mut(ui: &mut UiState) -> Option<&mut State> {
    if ui.varos_shell.enabled { ui.varos_panel_review.as_mut() } else { None }
}

/// Is the workspace the editor's presenter (Home included).
pub(crate) fn active(ui: &UiState) -> bool {
    state(ui).is_some()
}

/// Decide this frame's presenter once. Whenever the workspace is not drawing the editor (Home
/// covers it, or another presenter took over), its in-progress gestures, flyout and hit record
/// are cancelled here, so no stale interaction survives a switch back.
pub(crate) fn sync(app: &mut VectorcraftApp, on_home: bool) -> Presenter {
    let presenter = match state(&app.ui) {
        None => Presenter::Inactive,
        Some(_) if on_home => Presenter::Home,
        Some(_) => Presenter::Workspace,
    };
    if presenter != Presenter::Workspace
        && let Some(state) = app.ui.varos_panel_review.as_mut()
    {
        state.cancel_transient();
    }
    presenter
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Harness;
    use super::*;
    use serde_json::json;

    #[test]
    fn switching_the_shell_off_hands_queries_to_the_other_presenter_and_cancels_interaction() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("ui.panelReview", json!({"enabled": true})).unwrap();
        let state = app.ui.varos_panel_review.as_mut().unwrap();
        state.flyout = state.layout.group_of("layers").map(|group| group.id);
        assert_eq!(sync(&mut app, false), Presenter::Workspace);
        assert!(app.ui.varos_panel_review.as_ref().unwrap().flyout.is_some(), "drawing the workspace keeps its flyout");
        assert_eq!(sync(&mut app, true), Presenter::Home);
        assert!(app.ui.varos_panel_review.as_ref().unwrap().flyout.is_none(), "Home cancels the flyout");
        crate::varos::shell::set_enabled(&mut app, false);
        assert_eq!(sync(&mut app, false), Presenter::Inactive);
        assert!(app.ui.varos_panel_review.is_some(), "the saved layout survives the switch");
        assert_eq!(super::super::workspace::current(&app), None);
        assert!(super::super::workspace::command(&mut app, "window.workspace.manage", &json!({})).is_none());
        assert!(app.ui.dialog.is_none(), "the inactive workspace opens no dialog");
        assert!(!super::super::showing(&app, "tools"));
    }

    /// S8: the Varos shell switch is the one owner: off, the classic dock presents and the panel
    /// workspace neither draws nor answers; back on, the same workspace returns.
    #[test]
    fn turning_the_varos_shell_off_in_preferences_hands_the_editor_to_the_classic_dock() {
        let mut h = Harness::workspace();
        let layout = h.layout().clone();
        crate::prefs_dialog::open(&mut h.app, None);
        h.app.ui.dialog.as_mut().unwrap().fields.insert(crate::varos::shell::PREF_KEY.into(), json!(false));
        crate::prefs_dialog::apply(&mut h.app).unwrap();
        h.app.ui.dialog = None;
        assert!(!super::super::enabled(&h.app));
        h.app.ui.varos_panel_review.as_mut().unwrap().hits.clear();
        h.settle();
        assert!(h.state().hits.is_empty(), "the panel workspace draws nothing while the shell is off");
        assert!(super::super::command(&mut h.app, "window.panel.layout", &json!({})).is_none(), "nor answers panel commands");
        h.app.run("window.panel", json!({"panel": "color"})).unwrap();
        assert!(!h.visible("color"), "the classic dock handled Window › Color");
        assert!(crate::panel_docking::showing(&h.app.ui, "color"));
        crate::prefs_dialog::open(&mut h.app, None);
        h.app.ui.dialog.as_mut().unwrap().fields.insert(crate::varos::shell::PREF_KEY.into(), json!(true));
        crate::prefs_dialog::apply(&mut h.app).unwrap();
        h.app.ui.dialog = None;
        assert!(super::super::enabled(&h.app));
        h.settle();
        assert_eq!(h.layout(), &layout, "the saved workspace returns unchanged");
        assert!(!h.state().hits.is_empty());
    }
}
