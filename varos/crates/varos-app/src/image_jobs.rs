//! Phase 3 bounded import worker and revision-checked publication. Provisional UI owner review pending.
use std::path::PathBuf;
use serde::{Serialize,Deserialize};
use varos_core::images::{self,PlacementMode,LinkInfo};
use crate::{workspace::Workspace,app_command::SessionId,file_jobs::CancelFlag};
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default)]pub mode:PlacementMode,
    #[serde(default)]pub at:[f32;2],
    #[serde(default)]pub bounds:Option<[f32;4]>,
}
impl Default for Options {fn default()->Self {Self{mode:Default::default(),at:[0.;2],bounds:None}}}
#[derive(Clone,Debug,PartialEq)]
pub struct Job {pub sid:SessionId,pub ticket:u64,pub expected_rev:u64,pub path:PathBuf,pub options:Options,pub bridge:bool,pub cancel:CancelFlag}
#[derive(Clone,Debug,PartialEq)]
pub struct Done {pub job:Job,pub result:Result<(images::codec::Decoded,LinkInfo),String>}
pub fn execute(job:Job)->Done {
    let result=(||{
        if job.cancel.flag().load(std::sync::atomic::Ordering::Relaxed){return Err("Image placement cancelled".into());}
        let bytes=if job.bridge {let extension=job.path.extension().and_then(|e|e.to_str()).ok_or("Image source extension required")?;if !["png","jpg","jpeg","gif","webp","tif","tiff","bmp"].contains(&extension.to_ascii_lowercase().as_str()){return Err("Unsupported image extension".into());}varos_bridge::files::read_source(&job.path,extension,images::MAX_ORIGINAL).map_err(|e|e.reason)?}else{images::links::read_original(&job.path)?};
        let decoded=images::codec::decode(&bytes)?;let home=varos_bridge::files::account_home().ok();let link=images::links::locator(&job.path,&bytes,home.as_deref())?;
        Ok((decoded,link))
    })();Done{job,result}
}
pub fn complete(done:Done,ws:&mut Workspace)->Result<varos_bridge::Reply,String> {
    let job=&done.job;
    if job.cancel.flag().load(std::sync::atomic::Ordering::Relaxed){return Err("Image placement cancelled".into());}
    let s=ws.get_mut(job.sid).ok_or("Image target tab closed")?;
    if s.editor.rev!=job.expected_rev||!matches!(s.editor.drag,varos_core::editor::Drag::None)||!matches!(s.editor.ab_drag,varos_core::editor::AbDrag::None)||s.editor.transaction_open(){return Err("Image target changed; place again after finishing the gesture".into());}
    let (decoded,link)=done.result?;let notes=decoded.notes.clone();let before=s.editor.clone();
    let result=(||{let mut image=images::stage(&mut s.editor,decoded,job.options.at,job.options.mode,Some(link))?;
        if let Some([x,y,w,h])=job.options.bounds {image.xform=images::ImageAffine{a:w/image.px_w as f32,b:0.,c:0.,d:h/image.px_h as f32,e:x,f:y};}
        s.editor.try_execute(varos_core::EditCommand::Image(images::ImageEdit::Add{image,parent:None}))?;Ok(())})();
    if let Err(e)=result {s.editor=before;return Err(e);}
    Ok(varos_bridge::Reply::success(serde_json::json!({"rev":s.editor.rev,"image":s.editor.doc.images.last().map(|i|format!("image:{}",i.id)),"report":notes})))
}
