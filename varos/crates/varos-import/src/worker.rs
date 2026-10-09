//! Hosts re-enter their executable before GUI startup for an isolated foreign parser.
//! Hard ten-second deadline/cancellation kills the parser process; no window/EventLoop involved.
use crate::{Format, ImportOptions, ImportReport, MAX_BYTES};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
const ARG: &str = "--varos-import-worker";
const MAX_WIRE: usize = MAX_BYTES * 4 + 4096;
#[derive(serde::Serialize, serde::Deserialize)]
struct Request {
    bytes: Vec<u8>,
    format: Format,
    options: ImportOptions,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Reply {
    result: Result<(varos_core::model::Document, ImportReport), String>,
}
/// Call at the very beginning of desktop/CLI main. True means this invocation is worker-only.
pub fn worker_main() -> bool {
    if std::env::args_os().nth(1).is_none_or(|a| a != ARG) {
        return false;
    }
    let result = (|| {
        let mut data = Vec::new();
        std::io::stdin().take((MAX_WIRE + 1) as u64).read_to_end(&mut data).map_err(|e| e.to_string())?;
        if data.len() > MAX_WIRE {
            return Err("Import worker wire limit exceeded".into());
        }
        let request: Request = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
        crate::import_cancellable(&request.bytes, request.format, request.options, Arc::new(AtomicBool::new(false)))
    })();
    let reply = Reply { result };
    if let Ok(data) = serde_json::to_vec(&reply) {
        let _ = std::io::stdout().write_all(&data);
    }
    true
}
pub fn isolated_import(
    bytes: &[u8],
    format: Format,
    options: ImportOptions,
    cancel: &AtomicBool,
) -> Result<(varos_core::model::Document, ImportReport), String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Import cancelled".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err("Import input limit exceeded".into());
    }
    // DXF/SVG conversion loops have cooperative checkpoints. PDF's parser is isolated.
    if !matches!(format, Format::Pdf | Format::Ai) {
        let out = crate::import_cancellable(bytes, format, options, Arc::new(AtomicBool::new(false)))?;
        if cancel.load(Ordering::Acquire) {
            return Err("Import cancelled".into());
        }
        return Ok(out);
    }
    let data = serde_json::to_vec(&Request { bytes: bytes.to_vec(), format, options }).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg(ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let Some(mut input) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Import worker input unavailable".into());
    };
    let Some(output) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Import worker output unavailable".into());
    };
    let writer = std::thread::spawn(move || input.write_all(&data));
    let reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        output.take((MAX_WIRE + 1) as u64).read_to_end(&mut data).map(|_| data)
    });
    let status = loop {
        if cancel.load(Ordering::Acquire) || start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(if cancel.load(Ordering::Acquire) {
                "Import cancelled".into()
            } else {
                "Import parser exceeded ten-second limit".into()
            });
        }
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    let written = writer.join().map_err(|_| "Import worker writer failed")?;
    let data = reader.join().map_err(|_| "Import worker reader failed")?.map_err(|e| e.to_string())?;
    let status = status?;
    written.map_err(|e| e.to_string())?;
    if !status.success() || data.len() > MAX_WIRE {
        return Err("Import parser failed or output budget exceeded".into());
    }
    let reply: Reply = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Acquire) {
        return Err("Import cancelled".into());
    }
    let (doc, report) = reply.result?;
    let limits = varos_core::format::Limits::DEFAULT;
    varos_core::format::check_structure(&doc, &limits).map_err(|e| e.to_string())?;
    varos_core::format::validate(&doc, &limits).map_err(|e| e.to_string())?;
    varos_core::format::encode_model(&doc, &limits).map_err(|e| e.to_string())?;
    Ok((doc, report))
}
