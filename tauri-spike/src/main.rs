// Milestone 1: prove Tauri runs here + IPC (web panel ↔ Rust core) works.
// Next milestones: add a wgpu canvas + low-latency native input.

#[tauri::command]
fn ping(msg: String) -> String {
    format!("pong ← {}", msg)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
