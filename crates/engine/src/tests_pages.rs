use super::*;
use crate::cmd::fileio::{SaveMode, save_job, save_plan};
use serde_json::json;
fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":200,"height":100,"artboards":1})).unwrap();
    s
}
fn rect(s: &mut Session) {
    s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":20,"height":30})).unwrap();
}
fn bytes(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().unwrap()).unwrap()
}
#[test]
fn pages_commands_keep_per_page_history_selection_and_view_and_share_clipboard() {
    let mut s = session();
    rect(&mut s);
    let selected = s.doc().unwrap().selection.clone();
    s.doc_mut().unwrap().view =
        Some(vectorcraft_doc::SavedView { center: vectorcraft_geom::Point::new(44.0, 55.0), zoom: 2.0, name: String::new(), rotation: 0.0 });
    let view = s.doc().unwrap().view.clone();
    s.execute("edit.copy", &json!({})).unwrap();
    s.execute("page.add", &json!({})).unwrap();
    assert_eq!(s.project().unwrap().pages()[1].name, "Page 2");
    s.execute("page.switch", &json!({"page":1})).unwrap();
    assert!(s.doc().unwrap().history.undo.is_empty());
    s.execute("edit.paste", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), 1);
    s.execute("page.switch", &json!({"page":0})).unwrap();
    assert_eq!(s.doc().unwrap().selection, selected);
    assert_eq!(s.doc().unwrap().view, view);
    assert_eq!(s.doc().unwrap().history.undo.len(), 1);
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("page.switch", &json!({"page":1})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), 1);
    assert!(!s.doc().unwrap().selection.objects.is_empty());
}
#[test]
fn pages_rename_duplicate_move_delete_are_validated_without_polluting_art_history() {
    let mut s = session();
    rect(&mut s);
    assert!(s.execute("page.delete", &json!({})).is_err());
    s.execute("page.rename", &json!({"name":"Cover"})).unwrap();
    s.execute("page.duplicate", &json!({})).unwrap();
    let id = s.project().unwrap().pages()[1].id.clone();
    assert_eq!(s.project().unwrap().pages()[1].doc.doc.node_count(), s.doc().unwrap().doc.node_count());
    assert!(s.project().unwrap().pages()[1].doc.history.undo.is_empty());
    s.execute("page.move", &json!({"page":id,"index":0})).unwrap();
    assert_eq!(s.project().unwrap().active_page(), 1);
    assert_eq!(s.doc().unwrap().history.undo.len(), 1);
    assert!(s.execute("page.move", &json!({"index":99})).is_err());
    assert!(s.execute("page.rename", &json!({"name":" "})).is_err());
    s.execute("page.delete", &json!({"page":id})).unwrap();
    assert_eq!(s.project().unwrap().pages().len(), 1);
}
#[test]
fn pages_native_serialization_round_trips_every_canvas_and_active_page() {
    let mut s = session();
    rect(&mut s);
    s.execute("page.add", &json!({"name":"Back"})).unwrap();
    s.execute("page.switch", &json!({"page":1})).unwrap();
    rect(&mut s);
    let saved = s.execute("document.serialize", &json!({"format":"vectorcraft","preview":false})).unwrap();
    let file = vectorcraft_format::load_file(&bytes(&saved)).unwrap();
    assert_eq!(file.project.pages.len(), 2);
    assert_eq!(file.project.active, 1);
    let mut reopened = Session::new();
    reopened.execute("document.open", &json!({"name":"project.varos","dataBase64":saved["dataBase64"]})).unwrap();
    assert_eq!(reopened.project().unwrap().pages().len(), 2);
    assert_eq!(reopened.project().unwrap().active_page(), 1);
    assert!(!reopened.project().unwrap().is_dirty());
}
#[test]
fn pages_background_save_tracks_each_page_snapshot_even_if_the_front_changes() {
    let mut s = session();
    s.execute("page.add", &json!({})).unwrap();
    let p = json!({"path":"/tmp/pages-save-snapshot.varos","format":"vectorcraft","preview":false});
    let plan = save_plan(&s, SaveMode::SaveAs, &p).unwrap();
    let uid = s.doc().unwrap().uid;
    let job = save_job(&mut s, plan).unwrap();
    let encoded = job.encode().unwrap();
    assert_eq!(vectorcraft_format::load_file(&encoded.files[0].1).unwrap().project.pages.len(), 2);
    s.execute("page.switch", &json!({"page":1})).unwrap();
    rect(&mut s);
    job.complete(&mut s, uid);
    assert!(s.project().unwrap().is_dirty());
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(!s.project().unwrap().is_dirty());
}
#[test]
fn pages_pdf_defaults_to_active_and_all_pages_concatenates_artboards() {
    let mut s = session();
    s.execute("page.add", &json!({})).unwrap();
    for (all, count) in [(false, 1), (true, 2)] {
        let v = s.execute("document.serialize", &json!({"format":"pdf","allPages":all,"preserveEditing":false})).unwrap();
        assert_eq!(vectorcraft_pdf::info(&bytes(&v), None).unwrap().pages.len(), count);
    }
}

#[test]
fn pages_recovery_retains_names_background_content_and_active_page() {
    use crate::cmd::recovery::MemoryStore;
    let store = std::sync::Arc::new(MemoryStore::default());
    let mut s = session();
    s.recovery.set_store(store.clone());
    rect(&mut s);
    s.execute("page.rename", &json!({"name":"Cover"})).unwrap();
    s.execute("page.add", &json!({"name":"Back"})).unwrap();
    s.execute("page.switch", &json!({"page":1})).unwrap();
    let saved = s.execute("file.recovery.save", &json!({})).unwrap();
    assert_eq!(saved["saved"].as_array().unwrap().len(), 1, "{saved}");
    let again = s.execute("file.recovery.save", &json!({})).unwrap();
    assert!(again["saved"].as_array().unwrap().is_empty(), "{again}");
    drop(s);
    let mut restored = Session::new();
    restored.recovery.set_store(store);
    let result = restored.execute("file.recovery.restore", &json!({})).unwrap();
    assert_eq!(result["restored"].as_array().unwrap().len(), 1, "{result}");
    let p = restored.project().unwrap();
    assert_eq!(p.active_page(), 1);
    assert_eq!(p.pages()[0].name, "Cover");
    assert_eq!(p.pages()[1].name, "Back");
    assert!(p.pages()[0].doc.doc.node_count() > p.pages()[1].doc.doc.node_count());
    assert!(p.is_dirty());
}

#[test]
fn pages_all_pages_pdf_editing_attachment_reopens_as_a_project() {
    let mut s = session();
    s.execute("page.add", &json!({"name":"Second"})).unwrap();
    let pdf = s.execute("document.serialize", &json!({"format":"pdf","allPages":true,"preserveEditing":true})).unwrap();
    let mut reopened = Session::new();
    reopened.execute("document.open", &json!({"name":"project.pdf","dataBase64":pdf["dataBase64"]})).unwrap();
    assert_eq!(reopened.project().unwrap().pages().len(), 2);
    assert_eq!(reopened.project().unwrap().pages()[1].name, "Second");
}

#[test]
fn pages_save_a_copy_preserves_project_and_leaves_the_original_dirty() {
    let mut s = session();
    s.execute("page.rename", &json!({"name":"Original"})).unwrap();
    s.execute("page.add", &json!({"name":"Background"})).unwrap();
    let p = json!({"path":"/tmp/pages-copy.varos","format":"vectorcraft","preview":false});
    let plan = save_plan(&s, SaveMode::Copy, &p).unwrap();
    let uid = s.doc().unwrap().uid;
    let job = save_job(&mut s, plan).unwrap();
    let encoded = job.encode().unwrap();
    let file = vectorcraft_format::load_file(&encoded.files[0].1).unwrap();
    assert_eq!(file.project.pages[0].name, "Original");
    assert_eq!(file.project.pages[1].name, "Background");
    job.complete(&mut s, uid);
    assert!(s.project().unwrap().is_dirty());
    assert!(s.doc().unwrap().path.is_none());
}

#[test]
fn pages_package_and_document_info_include_background_canvases() {
    let mut s = session();
    s.execute("page.rename", &json!({"name":"Cover"})).unwrap();
    s.execute("page.add", &json!({"name":"Details"})).unwrap();
    s.execute("page.switch", &json!({"page":1})).unwrap();
    rect(&mut s);
    s.execute("page.switch", &json!({"page":0})).unwrap();
    s.doc_mut().unwrap().path = Some("/tmp/pages-package.varos".into());
    let info = s.execute("document.info", &json!({})).unwrap();
    assert_eq!(info["pages"].as_array().unwrap().len(), 2);
    assert!(info["pages"][1]["objects"].as_u64().unwrap() > info["pages"][0]["objects"].as_u64().unwrap());
    let package = s.prepare_package(&json!({"folder":"/tmp/pages-package-output","copyFonts":false})).unwrap();
    let mut files = std::collections::BTreeMap::new();
    package
        .write(&mut |path, bytes| {
            files.insert(path.to_owned(), bytes.to_vec());
            Ok(())
        })
        .unwrap();
    let native = files.values().find_map(|bytes| vectorcraft_format::load_file(bytes).ok()).unwrap();
    assert_eq!(native.project.pages.len(), 2);
    assert_eq!(native.project.pages[1].name, "Details");
    assert!(files.values().any(|bytes| String::from_utf8_lossy(bytes).contains("Details: 1 artboards")));
}

#[test]
fn pages_agent_attribution_reaches_inactive_pages() {
    let mut s = session();
    s.set_actor(Some(7));
    s.execute("page.add", &json!({})).unwrap();
    s.set_actor(None);
    s.execute("page.switch", &json!({"page":1})).unwrap();
    rect(&mut s);
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().actor, None);
    s.set_actor(Some(8));
    s.execute("page.switch", &json!({"page":0})).unwrap();
    rect(&mut s);
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().actor, Some(8));
    s.set_actor(None);
    assert!(s.project().unwrap().pages().iter().all(|p| p.doc.history.actor.is_none()));
}

#[test]
fn pages_native_dialog_defaults_save_the_project_in_the_current_version() {
    let mut s = session();
    s.execute("page.add", &json!({"name":"Back"})).unwrap();
    let described = s.execute("file.formatOptions", &json!({"format":"vectorcraft"})).unwrap();
    assert_eq!(described["options"]["version"]["default"], vectorcraft_format::VERSION);
    let options: serde_json::Map<String, Value> =
        described["options"].as_object().unwrap().iter().map(|(name, option)| (name.clone(), option["value"].clone())).collect();
    // The Save Options dialog passes these advertised values back to Save As.
    let plan = save_plan(&s, SaveMode::SaveAs, &json!({"path":"/tmp/pages-dialog.varos","options":options})).unwrap();
    let encoded = save_job(&mut s, plan).unwrap().encode().unwrap();
    let file = vectorcraft_format::load_file(&encoded.files[0].1).unwrap();
    assert_eq!(file.info.version, vectorcraft_format::VERSION);
    assert_eq!(file.project.pages.len(), 2);
}

#[test]
fn pages_duplicate_long_unicode_names_remains_saveable() {
    let mut s = session();
    s.execute("page.rename", &json!({"name":"🌍".repeat(1024)})).unwrap();
    s.execute("page.duplicate", &json!({})).unwrap();
    let copy = &s.project().unwrap().pages()[1];
    assert!(copy.name.ends_with(" copy"));
    assert!(copy.name.len() <= 4096);
    assert!(vectorcraft_format::save_project(&s.project().unwrap().snapshot(), &Default::default()).is_ok());
}

#[test]
fn pages_deleting_the_active_canvas_resets_tool_state() {
    let mut s = session();
    s.execute("page.add", &json!({})).unwrap();
    let resets = s.tool_resets;
    s.execute("page.delete", &json!({"page":0})).unwrap();
    assert_eq!(s.tool_resets, resets + 1);
    assert_eq!(s.project().unwrap().pages()[0].name, "Page 2");
    assert_eq!(s.project().unwrap().active_page(), 0);
    assert!(s.execute("page.delete", &json!({})).is_err());
    assert_eq!(s.tool_resets, resets + 1, "refusing the last page leaves the tool alone");
}
