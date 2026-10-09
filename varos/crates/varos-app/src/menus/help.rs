//! Lane F: Help commands use the same host path on each desktop surface.
use super::*;
pub(super) fn rows() -> Vec<Entry> {
    use crate::phase9::DesktopAction as A;
    vec![
        item("help.docs", "Varos Help", None, MenuCmd::Phase9(A::Help)),
        item("help.shortcuts", "Keyboard shortcuts", None, MenuCmd::Phase9(A::Shortcuts)),
        item("help.problem", "Report a problem", None, MenuCmd::Phase9(A::ReportProblem)),
    ]
}
