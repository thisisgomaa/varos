//! Request-side reference fields, audited against MCP tools.rs and engine cmd schemas.
//! A field's name alone is insufficient: `path`, `group`, `from`/`to`, even `id` also carry
//! authored strings. Only descend into declared request structures; plug-in/effect params,
//! metadata, labels and other user data are opaque. No engine dependency is needed here.

use serde_json::Value;

use super::walk_refs;

/// Decode a tool's declared object references, preserving its authored data. The public tool
/// shape and reference syntax stay unchanged. `command_run`, its aliases and batches use the
/// command's parameter semantics; the envelope's command id is never an object reference.
pub fn unqualify_tool(v: &mut Value, board: Option<&str>, tool: &str) -> Result<Option<String>, String> {
    let mut decoder = Decoder { named: board.map(str::to_string), given: board.is_some() };
    decoder.tool(v, tool)?;
    Ok(decoder.named)
}

struct Decoder {
    named: Option<String>,
    given: bool,
}

impl Decoder {
    fn fields(&mut self, v: &mut Value, keys: &[&str]) -> Result<(), String> {
        for key in keys {
            if let Some(value) = v.get_mut(*key) {
                walk_refs(value, &mut self.named, self.given, true)?;
            }
        }
        Ok(())
    }

    fn tool(&mut self, v: &mut Value, tool: &str) -> Result<(), String> {
        match tool {
            "command_run" => self.command_envelope(v, "id"),
            "run_command" | "invoke_menu" => self.command_envelope(v, "command"),
            "command_batch" => self.steps(v, "steps", "id"),
            "doc_inspect" | "inspect_document" | "apply_effect" | "pathfinder" | "transform" | "text_wrap" => self.fields(v, &["ids"]),
            "set_paint" => {
                self.fields(v, &["ids"])?;
                self.paint(v)
            }
            "draw_path" | "draw_shape" => self.paint(v),
            "add_text" => self.fields(v, &["path"]),
            // All other tool fields are authored values, coordinates, indexes or enums.
            _ => Ok(()),
        }
    }

    fn paint(&mut self, v: &mut Value) -> Result<(), String> {
        for key in ["fill", "stroke"] {
            if let Some(paint) = v.get_mut(key)
                && paint.as_object().is_some_and(|p| ["color", "none", "swatch", "gradient"].iter().any(|k| p.contains_key(*k)))
            {
                // tools::paint_params forwards a full command params object only with one of
                // these markers. Plain colour objects and gradient payloads are opaque.
                self.fields(paint, &["id", "ids"])?;
            }
        }
        Ok(())
    }

    fn command_envelope(&mut self, v: &mut Value, key: &str) -> Result<(), String> {
        let command = v.get(key).and_then(Value::as_str).unwrap_or("").to_string();
        if let Some(params) = v.get_mut("params") {
            self.command(params, &command)?;
        }
        Ok(())
    }

    fn steps(&mut self, v: &mut Value, key: &str, command_key: &str) -> Result<(), String> {
        if let Some(steps) = v.get_mut(key).and_then(Value::as_array_mut) {
            for step in steps {
                self.command_envelope(step, command_key)?;
            }
        }
        Ok(())
    }

    fn command(&mut self, v: &mut Value, command: &str) -> Result<(), String> {
        match command {
            "command.batch" | "action.batch" => return self.steps(v, "commands", "command"),
            // The plug-in id and all its parameters (including top-level shorthand) are opaque.
            "plugin.run" => return self.fields(v, &["ids"]),
            "plugin.info" | "plugin.remove" => return Ok(()),
            // A batch can invoke these MCP tools, including full paint parameter objects.
            "draw_path" | "draw_shape" => return self.paint(v),
            _ => {}
        }
        // cmd/mod.rs targets/given_objects use id/ids. effect.apply also accepts a string
        // effect identifier as `id`, but only when the explicit `effect` is absent.
        let effect_alias = matches!(command, "effect.apply" | "appearance.addEffect") && v.get("effect").and_then(Value::as_str).is_none();
        if !effect_alias {
            self.fields(v, &["id"])?;
        }
        self.fields(v, &["ids"])?;
        let keys: &[&str] = match command {
            "text.createInPath" | "object.objectsOnPath.attach" => &["path"],
            "text.thread.add" => &["from", "to"],
            "appearance.transfer" => &["source", "target"],
            "appearance.copyFrom" => &["source"],
            "layer.move" => &["target"],
            "layer.newSublayer" | "node.move" => &["parent"],
            "guide.add" | "guide.setLayer" => &["layer"],
            "livePaint.fill" | "livePaint.strokeEdge" | "livePaint.info" => &["group"],
            "object.intertwine.arrange" => &["member"],
            "document.exportForScreens" => &["layers", "assets"],
            "assets.remove" | "assets.export" => &["assets"],
            "assets.rename" => &["asset"],
            _ if command.starts_with("object.slice.") => &["slices"],
            _ => &[],
        };
        self.fields(v, keys)?;
        match command {
            "path.freehand" => {
                for key in ["extend", "connect", "redraw"] {
                    if let Some(record) = v.get_mut(key) {
                        self.fields(record, &["id"])?;
                    }
                }
            }
            "select.anchorsMany" => {
                if let Some(items) = v.get_mut("items").and_then(Value::as_array_mut) {
                    for item in items {
                        self.fields(item, &["id"])?;
                    }
                }
            }
            "dimension.create" => {
                if let Some(source) = v.get_mut("source") {
                    self.fields(source, &["node"])?;
                    for key in ["edge", "first", "second", "a", "b", "vertex"] {
                        if let Some(record) = source.get_mut(key) {
                            self.fields(record, &["node"])?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
