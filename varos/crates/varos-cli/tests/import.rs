use std::{path::PathBuf, process::Command};
fn root() -> PathBuf {
    std::env::temp_dir().join(format!("varos-lane-h-cli-{}", std::process::id()))
}
#[test]
fn foreign_import_cli_saves_native_and_refuses_native_source() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    let src = root.join("line.dxf");
    let out = root.join("line.vrs");
    std::fs::write(&src,"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n25.4\n21\n0\n0\nENDSEC\n0\nEOF\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .args(["import-dxf", src.to_str().unwrap(), "--out", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
    let original_output = std::fs::read(&out).unwrap();
    let retry = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .args(["import-dxf", src.to_str().unwrap(), "--out", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!retry.status.success());
    assert_eq!(std::fs::read(&out).unwrap(), original_output);
    let doc = varos_pdf::load_vrs(&out).unwrap();
    assert_eq!(doc.paths.len(), 1);
    assert!((doc.paths[0].anchors[1].p[0] - 72.).abs() < 0.001);
    let result = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .args(["import", out.to_str().unwrap(), "--out", root.join("refused.vrs").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!root.join("refused.vrs").exists());
    let bytes = std::fs::read(&src).unwrap();
    std::fs::write(root.join("renamed.vrs"), &bytes).unwrap();
    assert!(varos_pdf::load_vrs(&root.join("renamed.vrs")).is_err());
    assert_eq!(std::fs::read(&src).unwrap(), bytes);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn isolated_pdf_worker_is_headless_and_returns_native_geometry() {
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    let content = "1 0 0 rg 0 0 10 20 re f";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".into(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf += &format!("{} 0 obj\n{obj}\nendobj\n", i + 1);
    }
    let xref = pdf.len();
    pdf += "xref\n0 5\n0000000000 65535 f \n";
    for offset in offsets.iter().skip(1) {
        pdf += &format!("{offset:010} 00000 n \n");
    }
    pdf += &format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n");
    let root = std::env::temp_dir().join(format!("lane-h-pdf-worker-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("input.pdf");
    let output = root.join("output.vrs");
    std::fs::write(&source, pdf.as_bytes()).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .args(["import-pdf", source.to_str().unwrap(), "--out", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stdout));
    let doc = varos_pdf::load_vrs(&output).unwrap();
    assert_eq!(doc.paths.len(), 2);
    assert_eq!(doc.artboards[0].w, 100.);
    assert!(doc.paths.iter().any(|p| p.fill.solid() == Some([1., 0., 0., 1.])));
    assert_eq!(std::fs::read(&source).unwrap(), pdf.as_bytes());
    std::fs::remove_dir_all(root).unwrap();
}
