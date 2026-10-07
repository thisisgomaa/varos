#!/usr/bin/env python3
"""Materialize independent stock + combined feature workspaces; offline commands only."""
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[1]
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
stock=next(Path.home().glob('.cargo/registry/src/*/cosmic-text-0.19.0'))
for name in ['stock','combined']:
 d=out/name;(d/'src').mkdir(parents=True,exist_ok=True)
 if name=='combined':
  manifest=f'''[package]
name="p1b-combined"
version="0.0.0"
edition="2021"
[workspace]
[dependencies]
varos-text-spike={{path={str(root)!r}}}
resvg="=0.45.1"
[patch.crates-io]
cosmic-text={{path={str(root/'vendor/cosmic-text')!r}}}
'''
  source='pub fn engine() -> varos_text_spike::Engine { varos_text_spike::Engine::default() }\npub fn resvg_type() -> usize { core::mem::size_of::<resvg::usvg::Options>() }\n'
  (d/'src/lib.rs').write_text(source)
  (d/'src/main.rs').write_text((root/'scripts/render_svg.rs').read_text())
 else:
  manifest=f'''[package]
name="p1b-stock"
version="0.0.0"
edition="2021"
[workspace]
[dependencies]
cosmic-text={{path={str(stock)!r},default-features=false,features=["no_std"]}}
fontdb={{version="=0.23.0",default-features=false}}
unicode-linebreak="=0.1.5"
'''
  source='''use cosmic_text::*;
fn main() {
 let mut db=fontdb::Database::new();
 db.load_font_data(include_bytes!("FONT_INTER").to_vec());
 db.load_font_data(include_bytes!("FONT_PLEX").to_vec());
 let mut fs=FontSystem::new_with_locale_and_db("en-US".into(),db);
 for text in ["السعر ١٢٣٫٤٥ ج.م. (USD 12.50)","A\\u{2067}شعار 12\\u{2069}Z"] {
  for size in [48.,200.] {for width in [120.,600.] {
   let attrs=AttrsList::new(&Attrs::new().family(Family::Name("IBM Plex Sans Arabic")));
   let shape=ShapeLine::new(&mut fs,text,&attrs,Shaping::Advanced,4);
   let legal:Vec<_>=unicode_linebreak::linebreaks(text).map(|(i,_)|i).collect();
   for line in shape.layout(size,Some(width),Wrap::Word,None,None,Hinting::Disabled) {
    let mut end=line.glyphs.iter().map(|g|g.end).max().unwrap_or(0);
    for c in text[end..].chars() {if c==' ' {end+=1;}else{break;}}
    println!("stock {text:?} {size}pt/{width}pt end={end} legal={}",end==text.len()||legal.contains(&end));
   }
  }}
 }
}
'''.replace('FONT_INTER',str(root/'../../crates/varos-app/assets/fonts/Inter-Regular.ttf')).replace('FONT_PLEX',str(root/'../../crates/varos-app/assets/fonts/IBMPlexSansArabic-Regular.ttf'))
  (d/'src/main.rs').write_text(source)
 (d/'Cargo.toml').write_text(manifest)
 print(d)
