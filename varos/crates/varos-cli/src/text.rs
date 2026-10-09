//! Lane G: headless text creation/edit. Advanced run/paragraph edits also use `apply` API 1.2.
use super::{json, required, same_file, write_output, Editor, Limits, OsString, PathBuf, Value};
use varos_core::{text::TextBoxKind, EditCommand};
pub fn run(verb: &str, args: Vec<OsString>) -> Result<Value, String> {
    let mut args = args.into_iter();
    let input = PathBuf::from(args.next().ok_or("missing document")?);
    let mut out = None;
    let mut source = None;
    let mut id = None;
    let mut origin = None;
    let mut area = None;
    let mut size = None;
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "option must be UTF-8")?;
        let value = args.next().ok_or_else(|| format!("missing {key} value"))?;
        if key == "--out" {
            out = Some(PathBuf::from(value));
            continue;
        }
        let value = value.into_string().map_err(|_| "value must be UTF-8")?;
        match key.as_str() {
            "--text" => source = Some(value),
            "--id" => id = Some(value.parse::<u32>().map_err(|_| "invalid text id")?),
            "--size" => size = Some(value.parse::<f32>().map_err(|_| "invalid font size")?),
            "--at" => origin = Some(numbers::<2>(&value)?),
            "--area" => area = Some(numbers::<4>(&value)?),
            _ => return Err(format!("unknown text option {key}")),
        }
    }
    let out = required(out, "--out")?;
    if same_file(&input, &out)? {
        return Err("text output must differ from input; use apply --in-place for explicit replacement".into());
    }
    let source = required(source, "--text")?;
    let mut editor = Editor::new();
    editor.replace_doc(varos_pdf::load_vrs(&input)?);
    let mut text = if verb == "set-text" {
        let id = required(id, "--id")?;
        let old = editor.doc.text_boxes.iter().find(|t| t.id == id).ok_or("unknown text id")?.clone();
        let mut session = varos_text_layout::edit::EditSession::new(old);
        session.select_all();
        session.insert(&source)?;
        session.draft
    } else {
        if id.is_some() {
            return Err("--id is only for set-text".into());
        }
        varos_text_layout::default_text(&source, origin.unwrap_or([0., 0.]))?
    };
    if let Some(p) = origin {
        text.frame = p;
    }
    if let Some(r) = area {
        text.box_kind = TextBoxKind::Area(r);
    }
    if let Some(size) = size {
        for run in &mut text.runs {
            run.style.size = size;
        }
    }
    let id = if verb == "set-text" {
        let id = text.id;
        editor.try_execute(EditCommand::SetText { id, text })?;
        id
    } else {
        editor.try_execute_created(EditCommand::AddText { text, parent: None })?
    };
    let bytes = varos_pdf::write_pdf_checked(&editor.doc, &Limits::DEFAULT)?;
    write_output(&out, &bytes)?;
    Ok(
        json!({"out":out.to_string_lossy(),"text_id":id,"source":editor.doc.text_boxes.iter().find(|t|t.id==id).map(|t|t.source())}),
    )
}
fn numbers<const N: usize>(value: &str) -> Result<[f32; N], String> {
    let values = value
        .split(',')
        .map(|v| v.parse::<f32>().map_err(|_| "invalid coordinate".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    values.try_into().map_err(|_| format!("expected {N} comma-separated coordinates"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_and_replace_arabic_cli() {
        let root = std::env::temp_dir().join(format!("varos-text-cli-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let input = root.join("input.vrs");
        let added = root.join("added.vrs");
        let changed = root.join("changed.vrs");
        for p in [&input, &added, &changed] {
            let _ = std::fs::remove_file(p);
        }
        varos_pdf::save_vrs(&varos_core::model::Document::default(), &input).unwrap();
        let r = run(
            "add-text",
            vec![
                input.into_os_string(),
                "--text".into(),
                "مرحبا".into(),
                "--at".into(),
                "10,60".into(),
                "--out".into(),
                added.clone().into_os_string(),
            ],
        )
        .unwrap();
        let id = r["text_id"].as_u64().unwrap();
        run(
            "set-text",
            vec![
                added.into_os_string(),
                "--id".into(),
                id.to_string().into(),
                "--text".into(),
                "سلام Varos".into(),
                "--out".into(),
                changed.clone().into_os_string(),
            ],
        )
        .unwrap();
        assert_eq!(varos_pdf::load_vrs(&changed).unwrap().text_boxes[0].source(), "سلام Varos");
        let _ = std::fs::remove_dir_all(root);
    }
}
