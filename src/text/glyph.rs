#[derive(Clone, Copy)]
pub(super) struct Glyph {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

pub(super) struct RasterizedGlyph {
    pub(super) character: char,
    pub(super) data: Vec<u8>,
    pub(super) glyph: Glyph,
}
