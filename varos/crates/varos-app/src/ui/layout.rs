//! Shell state only; document tabs, layer-node expansion and Start page stay outside this snapshot.
use super::Ui;
use varos_app::storage::layout::Layout;

impl Ui {
    pub fn shell_layout(&self) -> Layout {
        Layout { tree: self.shell.layout_value(), show_rail: self.show_rail, show_control_bar: self.show_dock }
    }

    pub fn restore_shell_layout(&mut self, layout: Layout) {
        self.shell = varos_app::shell::ShellState::from_layout_value(layout.tree)
            .unwrap_or_else(varos_app::shell::ShellState::standard);
        self.show_rail = layout.show_rail;
        self.show_dock = layout.show_control_bar;
        self.panel_column = None; // derived from restored splits on the first workspace pass
        self.ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests;
