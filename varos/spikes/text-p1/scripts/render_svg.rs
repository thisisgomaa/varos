fn render(input:&std::path::Path,output:&std::path::Path,scale:f32) {
    let svg=std::fs::read_to_string(input).unwrap();
    let tree=resvg::usvg::Tree::from_str(&svg,&resvg::usvg::Options::default()).unwrap();
    let size=tree.size();
    let mut pixels=resvg::tiny_skia::Pixmap::new((size.width()*scale).ceil() as u32,(size.height()*scale).ceil() as u32).unwrap();
    resvg::render(&tree,resvg::tiny_skia::Transform::from_scale(scale,scale),&mut pixels.as_mut());
    pixels.save_png(output).unwrap();
}
fn main() {
    let args:Vec<_>=std::env::args().collect();
    let input=std::path::Path::new(&args[1]);let output=std::path::Path::new(&args[2]);
    let scale:f32=args.get(3).map_or(2.,|s|s.parse().unwrap());
    let mut count=0;
    if input.is_dir() {
        std::fs::create_dir_all(output).unwrap();
        let mut paths:Vec<_>=std::fs::read_dir(input).unwrap().filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|x|x=="svg")).collect();paths.sort();
        for path in paths {render(&path,&output.join(path.file_stem().unwrap()).with_extension("png"),scale);count+=1;}
    } else {render(input,output,scale);count+=1;}
    println!("resvg 0.45.1 / tiny-skia 0.11.4 / @{scale}x / {count} files");
}
