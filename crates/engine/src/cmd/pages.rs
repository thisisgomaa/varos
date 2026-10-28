use super::*;
pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "page.list", "List Pages", [], None, "{} → {pages: [{page_id, page_name, active, index, artboards}], active_page}", has_doc, |s, _| Ok(s.project()?.list_pages())),
        cmd!("page.add", "Add Page", [], None, "{name?} add an independent canvas", has_doc, |s, p| crate::project::command(s, p, "add")),
        cmd!("page.rename", "Rename Page", [], None, "{page?: id|index, name} rename a page", has_doc, |s, p| crate::project::command(
            s, p, "rename"
        )),
        cmd!("page.duplicate", "Duplicate Page", [], None, "{page?: id|index, name?} duplicate a canvas with fresh history", has_doc, |s, p| {
            crate::project::command(s, p, "duplicate")
        }),
        cmd!("page.delete", "Delete Page", [], None, "{page?: id|index} delete a page (last page refused)", has_doc, |s, p| crate::project::command(
            s, p, "delete"
        )),
        cmd!("page.move", "Move Page", [], None, "{page?: id|index, index: zero-based destination} reorder pages", has_doc, |s, p| {
            crate::project::command(s, p, "move")
        }),
        cmd!(
            "page.switch",
            "Switch Page",
            [],
            None,
            "{page?: id|index} switch canvas without an undo step",
            has_doc,
            |s, p| crate::project::command(s, p, "switch")
        ),
    ]
}
