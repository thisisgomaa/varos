//! Headless outlined-glyph prototypes; not connected to production exporters.
//! Independent glyph paths preserve opposite font winding conventions.
use crate::outlines::{Command, Outline};
use std::fmt::Write;
pub fn svg(outlines: &[Outline], width: f32, height: f32) -> String {
    svg_at(outlines, width, height, [0., 0.])
}
pub fn svg_at(outlines: &[Outline], width: f32, height: f32, origin: [f32; 2]) -> String {
    let mut s=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    writeln!(s, "<g transform=\"translate({} {})\">", origin[0], origin[1]).unwrap();
    for o in outlines {
        s.push_str("<path fill=\"black\" fill-rule=\"nonzero\" d=\"");
        for c in &o.commands {
            match c {
                Command::Move(p) => write!(s, "M{} {} ", p[0], p[1]),
                Command::Line(p) => write!(s, "L{} {} ", p[0], p[1]),
                Command::Cubic(a, b, c) => write!(s, "C{} {} {} {} {} {} ", a[0], a[1], b[0], b[1], c[0], c[1]),
                Command::Close => write!(s, "Z "),
            }
            .unwrap();
        }
        s.push_str("\"/>\n");
    }
    s.push_str("</g>\n</svg>\n");
    s
}
pub fn pdf(outlines: &[Outline], width: f32, height: f32) -> Vec<u8> {
    pdf_at(outlines, width, height, [0., 0.])
}
pub fn pdf_at(outlines: &[Outline], width: f32, height: f32, origin: [f32; 2]) -> Vec<u8> {
    let mut commands = format!("q\n1 0 0 -1 0 {height} cm\n1 0 0 1 {} {} cm\n0 g\n", origin[0], origin[1]);
    for o in outlines {
        for c in &o.commands {
            match c {
                Command::Move(p) => writeln!(commands, "{} {} m", p[0], p[1]),
                Command::Line(p) => writeln!(commands, "{} {} l", p[0], p[1]),
                Command::Cubic(a, b, c) => {
                    writeln!(commands, "{} {} {} {} {} {} c", a[0], a[1], b[0], b[1], c[0], c[1])
                }
                Command::Close => writeln!(commands, "h"),
            }
            .unwrap();
        }
        commands.push_str("f\n");
    }
    commands.push_str("Q\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << >> /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{}endstream", commands.len(), commands),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    for (i, o) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(pdf, "{} 0 obj\n{}\nendobj\n", i + 1, o).unwrap();
    }
    let xref = pdf.len();
    write!(pdf, "xref\n0 5\n0000000000 65535 f \n").unwrap();
    for offset in &offsets[1..] {
        writeln!(pdf, "{offset:010} 00000 n ").unwrap();
    }
    write!(pdf, "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").unwrap();
    pdf.into_bytes()
}
