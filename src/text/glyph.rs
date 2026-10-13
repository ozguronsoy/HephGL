#[derive(Clone, Copy)]
pub struct Glyph {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct RasterizedGlyph {
    pub character: char,
    pub data: Vec<u8>,
    pub glyph: Glyph,
}
