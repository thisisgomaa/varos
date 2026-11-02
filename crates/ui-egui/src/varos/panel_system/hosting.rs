//! The rendering policy a classic content body (the Tools strip, the Control bar) receives from
//! whoever renders it. It is passed explicitly, so upstream renderers never look up which
//! presenter owns the editor.

/// Where a classic content body renders for this call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hosting {
    /// Its own chrome: upstream's docks and bars, or the Varos box shell.
    #[default]
    Own,
    /// A panel-workspace host, which owns moving, scrolling, sizing and hiding.
    Workspace,
}
