//! Lane A: vector Appearance forms, nested transparency groups and alpha SMask.
use super::*;
use varos_core::{
    appearance_scene::paint_paths,
    model::{GroupRole, NodeKind},
};
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_tree(
    doc: &Document,
    page: &PageSpec,
    c: &mut Content,
    pdf: &mut Pdf,
    ids: &mut Alloc,
    gss: &mut Vec<Gs>,
    knocks: &mut Vec<Knock>,
    knock_pool: &mut KnockGs,
    gradients: &mut crate::gradient::Pool,
    images: &[crate::image_write::DrawImage],
    cancel: &AtomicBool,
) -> Result<Vec<(String, Ref)>, ExportError> {
    struct Builder<'a> {
        doc: &'a Document,
        page: &'a PageSpec,
        pdf: &'a mut Pdf,
        ids: &'a mut Alloc,
        gss: &'a mut Vec<Gs>,
        knocks: &'a mut Vec<Knock>,
        knock_pool: &'a mut KnockGs,
        gradients: &'a mut crate::gradient::Pool,
        images: &'a [crate::image_write::DrawImage],
        cancel: &'a AtomicBool,
        forms: Vec<(String, Ref)>,
        states: Vec<(String, Ref)>,
    }
    impl Builder<'_> {
        fn node(&mut self, id: u32) -> Result<Option<String>, ExportError> {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(ExportError::Cancelled);
            }
            let Some(n) = self.doc.node(id) else { return Ok(None) };
            if n.hidden {
                return Ok(None);
            }
            let n = n.clone();
            let mut content = Content::new();
            let [x, y, _w, h] = self.page.rect;
            let t = |p: [f32; 2]| (p[0] - x, h - (p[1] - y));
            match n.kind {
                NodeKind::Path(pid) => {
                    let Some(pi) = self.doc.pidx(pid) else { return Ok(None) };
                    let source = &self.doc.paths[pi];
                    let entries = if source.stack.is_empty() { vec![source.clone()] } else { paint_paths(source) };
                    for p in &entries {
                        let Some(d) = drawable(self.doc, pi, p) else { continue };
                        if !crate::gradient::paint(self.doc, &d, &mut content, self.pdf, self.ids, self.gradients, &t) {
                            paint(&mut content, self.gss, self.knocks, self.knock_pool, self.ids, &d, &t);
                        }
                    }
                    if !source.stack.is_empty() && source.opacity < 1. {
                        let gs = gs_name(self.gss, self.ids, source.opacity, source.opacity);
                        let r = self.form(content.finish().to_vec(), false, true)?;
                        content = Content::new();
                        content.set_parameters(Name(gs.as_bytes())).x_object(Name(r.as_bytes()));
                    }
                }
                NodeKind::Image(pid) => {
                    if let Some(im) = self.images.iter().find(|i| i.id == pid) {
                        paint_item(
                            &mut content,
                            self.gss,
                            self.knocks,
                            self.knock_pool,
                            self.ids,
                            &Item::Image(im),
                            &t,
                            self.doc,
                        );
                    }
                }
                _ => {
                    if n.role == GroupRole::Clip {
                        if let Some(mask) = n.mask_child {
                            for pid in self.doc.node_paths(mask) {
                                if let Some(pi) = self.doc.pidx(pid) {
                                    emit_rings(&mut content, &self.doc.paths[pi], &self.doc.unit_xform(pid), &t);
                                }
                            }
                        }
                        content.clip_even_odd().end_path();
                    }
                    let soft = if n.role == GroupRole::MaskAlpha {
                        let name = n.mask_child.map(|id| self.node(id)).transpose()?.flatten();
                        Some(match name {
                            Some(name) => name,
                            None => self.form(vec![], false, true)?,
                        })
                    } else {
                        None
                    };
                    for child in n.children.iter().rev().filter(|id| Some(**id) != n.mask_child) {
                        if let Some(name) = self.node(*child)? {
                            content.x_object(Name(name.as_bytes()));
                        }
                    }
                    if let Some(mask_name) = soft {
                        let reference = self
                            .forms
                            .iter()
                            .find(|(name, _)| name == &mask_name)
                            .map(|(_, r)| *r)
                            .ok_or_else(|| ExportError::InvalidDocument("missing alpha form".into()))?;
                        let state = self.ids.next();
                        let name = format!("AM{}", state.get());
                        self.pdf
                            .ext_graphics(state)
                            .soft_mask()
                            .subtype(pdf_writer::types::MaskType::Alpha)
                            .group(reference);
                        self.states.push((name.clone(), state));
                        // Mask the completed group once, including overlapping children.
                        let painted = self.form(content.finish().to_vec(), false, true)?;
                        content = Content::new();
                        content.set_parameters(Name(name.as_bytes())).x_object(Name(painted.as_bytes()));
                    }
                }
            }
            let form = self.form(content.finish().to_vec(), false, true)?;
            if let Some(look) = n.look {
                if look.opacity < 1. {
                    let gs = gs_name(self.gss, self.ids, look.opacity, look.opacity);
                    let mut wrapper = Content::new();
                    wrapper.set_parameters(Name(gs.as_bytes())).x_object(Name(form.as_bytes()));
                    return self.form(wrapper.finish().to_vec(), false, true).map(Some);
                }
            }
            Ok(Some(form))
        }
        fn form(&mut self, data: Vec<u8>, knockout: bool, isolated: bool) -> Result<String, ExportError> {
            let r = self.ids.next();
            let name = format!("Ap{}", r.get());
            let mut form = self.pdf.form_xobject(r, &data);
            form.bbox(Rect::new(0., 0., self.page.rect[2], self.page.rect[3]));
            form.group().transparency().isolated(isolated).knockout(knockout).color_space().device_rgb();
            {
                let mut res = form.resources();
                {
                    let mut xo = res.x_objects();
                    for (name, r) in &self.forms {
                        xo.pair(Name(name.as_bytes()), *r);
                    }
                    for im in self.images {
                        xo.pair(Name(im.name.as_bytes()), im.r);
                    }
                    for (i, k) in self.knocks.iter().enumerate() {
                        xo.pair(Name(format!("Fx{i}").as_bytes()), k.r);
                    }
                    for (i, g) in self.gradients.iter().enumerate() {
                        if let Some(r) = g.form {
                            xo.pair(Name(format!("GrForm{i}").as_bytes()), r);
                        }
                    }
                }
                {
                    let mut gs = res.ext_g_states();
                    for (i, g) in self.gss.iter().enumerate() {
                        gs.pair(Name(format!("GS{i}").as_bytes()), g.r);
                    }
                    for (name, r) in &self.states {
                        gs.pair(Name(name.as_bytes()), *r);
                    }
                    for (i, g) in self.gradients.iter().enumerate() {
                        if let Some(r) = g.state {
                            gs.pair(Name(format!("GrGS{i}").as_bytes()), r);
                        }
                    }
                }
                {
                    let mut sh = res.shadings();
                    for (i, g) in self.gradients.iter().enumerate() {
                        if let Some(r) = g.colour {
                            sh.pair(Name(format!("Gr{i}").as_bytes()), r);
                        }
                    }
                }
            }
            form.finish();
            self.forms.push((name.clone(), r));
            Ok(name)
        }
    }
    let mut builder = Builder {
        doc,
        page,
        pdf,
        ids,
        gss,
        knocks,
        knock_pool,
        gradients,
        images,
        cancel,
        forms: vec![],
        states: vec![],
    };
    for id in doc.roots.iter().rev() {
        if let Some(name) = builder.node(*id)? {
            c.x_object(Name(name.as_bytes()));
        }
    }
    Ok(builder.forms)
}
