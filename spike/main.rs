use rex::font::backend::ttf_parser::TtfMathFont;
use rex::font::common::GlyphId;
use rex::layout::engine::LayoutBuilder;
use rex::render::{Backend, Cursor, FontBackend, GraphicsBackend, Renderer, RGBA};
use std::fmt::Write;

struct Collect { glyphs: Vec<(f64, f64, u16, f64)>, rules: Vec<(f64, f64, f64, f64)> }
impl<'a> FontBackend<TtfMathFont<'a>> for Collect {
    fn symbol(&mut self, pos: Cursor, gid: GlyphId, scale: f64, _f: &TtfMathFont<'a>) {
        self.glyphs.push((pos.x, pos.y, gid.into(), scale));
    }
}
impl GraphicsBackend for Collect {
    fn rule(&mut self, pos: Cursor, w: f64, h: f64) { self.rules.push((pos.x, pos.y, w, h)); }
    fn begin_color(&mut self, _c: RGBA) {}
    fn end_color(&mut self) {}
}
impl<'a> Backend<TtfMathFont<'a>> for Collect {}

struct Path(String);
impl ttf_parser::OutlineBuilder for Path {
    fn move_to(&mut self, x: f32, y: f32) { write!(self.0, "M{} {}", x, -y).unwrap(); }
    fn line_to(&mut self, x: f32, y: f32) { write!(self.0, "L{} {}", x, -y).unwrap(); }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) { write!(self.0, "Q{} {} {} {}", x1, -y1, x, -y).unwrap(); }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) { write!(self.0, "C{} {} {} {} {} {}", x1, -y1, x2, -y2, x, -y).unwrap(); }
    fn close(&mut self) { self.0.push('Z'); }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let font_path = &args[1];
    let tex = &args[2];
    let bytes = std::fs::read(font_path).unwrap();
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    let upem = face.units_per_em() as f64;
    let font = TtfMathFont::new(face).unwrap();
    let engine = LayoutBuilder::new(&font).font_size(12.0).build();
    let nodes = rex::parser::parse(tex).unwrap();
    let layout = engine.layout(&nodes).unwrap();
    let bbox = layout.full_bounding_box();
    let mut c = Collect { glyphs: vec![], rules: vec![] };
    Renderer::new().render(&layout, &mut c);
    let face = font.font();
    let mut svg = String::new();
    write!(svg, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}">"#,
        bbox.x_min, bbox.y_min, bbox.width(), bbox.height(), bbox.width()*4.0, bbox.height()*4.0).unwrap();
    for (x, y, gid, scale) in &c.glyphs {
        let mut p = Path(String::new());
        face.outline_glyph(ttf_parser::GlyphId(*gid), &mut p);
        let s = scale / upem;
        write!(svg, r#"<path transform="translate({:.3} {:.3}) scale({:.6})" d="{}"/>"#, x, y, s, p.0).unwrap();
    }
    for (x, y, w, h) in &c.rules {
        write!(svg, r#"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}"/>"#, x, y, w, h).unwrap();
    }
    svg.push_str("</svg>");
    eprintln!("{} glyphs, {} rules, bbox {:?}", c.glyphs.len(), c.rules.len(), (bbox.x_min, bbox.y_min, bbox.width(), bbox.height()));
    println!("{}", svg);
}
