//! Provisional image workflow routing. All artwork publication uses checked commands.
use varos_core::{
    images::{self, ImageEdit, PlacementMode},
    trace::{TraceMode, TraceOptions},
    EditCommand, Editor,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Relink(u32),
    Update(u32),
    Embed(u32),
    Unembed(u32),
    GoTo(u32),
    Crop(u32, [f32; 4]),
    Transform(u32, images::ImageAffine, f32),
    Rasterize(u32, f32, Option<[f32; 4]>),
    Trace(u32, TraceMode),
    EffectsPpi(f32),
    Package,
}
pub fn run(ed: &mut Editor, action: Action) -> Result<(), String> {
    let home = varos_bridge::files::account_home().ok();
    match action {
        Action::GoTo(id) => {
            ed.objsel.clear();
            ed.objsel.insert(id);
        }
        Action::Embed(id) => {
            ed.try_execute(EditCommand::Image(ImageEdit::Mode { id, mode: PlacementMode::Embed, link: None }))?
        }
        Action::Update(id) => images::links::update(ed, &[id], home.as_deref())?,
        Action::Relink(id) => {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Relink image")
                .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp"])
                .pick_file()
            {
                images::links::relink(ed, id, &path, home.as_deref())?;
            }
        }
        Action::Unembed(id) => {
            if let Some(path) = rfd::FileDialog::new().set_title("Unembed original image").save_file() {
                images::links::unembed(ed, id, &path, home.as_deref())?;
            }
        }
        Action::Crop(id, bounds) => ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds }))?,
        Action::Transform(id, xform, opacity) => {
            ed.try_execute(EditCommand::Image(ImageEdit::Transform { id, xform, opacity }))?
        }
        Action::Rasterize(id, ppi, bg) => varos_raster::images::rasterize_object(ed, id, ppi, bg)?,
        Action::Trace(id, mode) => {
            images::trace::expand(ed, id, &TraceOptions { mode, ..Default::default() })?;
        }
        Action::EffectsPpi(ppi) => ed.try_execute(EditCommand::Image(ImageEdit::EffectsPpi(ppi)))?,
        Action::Package => {
            if let Some(parent) = rfd::FileDialog::new().set_title("Package destination parent").pick_folder() {
                varos_pdf::package::package(&ed.doc, &ed.blobs, &parent.join("Varos Package"))?;
            }
        }
    }
    Ok(())
}
