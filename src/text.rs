use std::{collections::HashMap, path::Path};

use fontdue::{
    FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};

/// Represents a vertex used for text rendering, containing a 2D position, texture coordinates, and
/// an RGBA color.
pub type TextVertex = [f32; 8];

/// Represents an error that can occur while loading or processing text resources.
#[derive(Debug)]
pub enum TextError {
    /// An error occurred while reading font data from disk.
    Io(String),
    /// An error occurred while parsing or rasterizing a font.
    Font(String),
}

struct Glyph {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
}

/// Represents a loaded font.
pub struct Font {
    /// The parsed font data used for layout and glyph rasterization.
    font: fontdue::Font,
    /// The font size used when rasterizing glyphs.
    size: f32,
    /// The width of the glyph atlas in pixels.
    atlas_width: u32,
    /// The height of the glyph atlas in pixels.
    atlas_height: u32,
    /// The single-channel pixel data of the glyph atlas.
    atlas_data: Vec<u8>,
    /// The atlas metadata for each rasterized glyph.
    glyphs: HashMap<char, Glyph>,
}

impl Font {
    /// Loads a font from a `.tff` file.
    pub fn from_file<P: AsRef<Path>>(path: P, size: f32) -> Result<Self, TextError> {
        let data = std::fs::read(path)?;
        let font = fontdue::Font::from_bytes(data, FontSettings::default())?;
        Self::new(font, size)
    }

    /// Gets the width of the glyph atlas in pixels.
    pub fn atlas_width(&self) -> u32 {
        self.atlas_width
    }

    /// Gets the height of the glyph atlas in pixels.
    pub fn atlas_height(&self) -> u32 {
        self.atlas_height
    }

    /// Gets the single-channel pixel data of the glyph atlas.
    pub fn atlas_data(&self) -> &[u8] {
        &self.atlas_data
    }

    /// Calculates the vertices of the provided text.
    pub fn vertices(
        &self,
        text: &str,
        position: [f32; 2],
        viewport_size: [f32; 2],
        color: [f32; 4],
    ) -> Vec<TextVertex> {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position[0],
            y: position[1],
            ..LayoutSettings::default()
        });
        layout.append(&[&self.font], &TextStyle::new(text, self.size, 0));

        let mut vertices = Vec::with_capacity(layout.glyphs().len() * 6);
        for positioned in layout.glyphs() {
            if positioned.width == 0 || positioned.height == 0 {
                continue;
            }

            let Some(glyph) = self
                .glyphs
                .get(&positioned.parent)
                .or_else(|| self.glyphs.get(&'?'))
            else {
                continue;
            };

            let mut x0 = positioned.x;
            let mut y0 = positioned.y;
            let mut x1 = x0 + positioned.width as f32;
            let mut y1 = y0 + positioned.height as f32;

            x0 = x0 / viewport_size[0] * 2.0 - 1.0;
            x1 = x1 / viewport_size[0] * 2.0 - 1.0;
            y0 = y0 / viewport_size[1] * 2.0 - 1.0;
            y1 = y1 / viewport_size[1] * 2.0 - 1.0;

            let [u0, v0] = glyph.uv_min;
            let [u1, v1] = glyph.uv_max;
            vertices.extend_from_slice(&[
                [x0, y0, u0, v0, color[0], color[1], color[2], color[3]],
                [x1, y0, u1, v0, color[0], color[1], color[2], color[3]],
                [x1, y1, u1, v1, color[0], color[1], color[2], color[3]],
                [x0, y0, u0, v0, color[0], color[1], color[2], color[3]],
                [x1, y1, u1, v1, color[0], color[1], color[2], color[3]],
                [x0, y1, u0, v1, color[0], color[1], color[2], color[3]],
            ]);
        }

        vertices
    }

    fn new(font: fontdue::Font, size: f32) -> Result<Self, TextError> {
        const ATLAS_WIDTH: usize = 512;
        const PADDING: usize = 1;

        struct RasterizedGlyph {
            character: char,
            width: usize,
            height: usize,
            data: Vec<u8>,
            x: usize,
            y: usize,
        }

        // Rasterize ASCII characters.
        // TODO: Add unicode support.
        let mut rasterized = Vec::with_capacity(95);
        for character in '\x20'..='\x7E' {
            let (metrics, data) = font.rasterize(character, size);
            rasterized.push(RasterizedGlyph {
                character,
                width: metrics.width,
                height: metrics.height,
                data,
                x: 0,
                y: 0,
            });
        }

        let mut x = PADDING;
        let mut y = PADDING;
        let mut row_height = 0;
        for glyph in &mut rasterized {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }

            if x + glyph.width + PADDING > ATLAS_WIDTH {
                // Current row is full, add a new row.
                x = PADDING;
                y += row_height + PADDING;
                row_height = 0;
            }

            glyph.x = x;
            glyph.y = y;

            x += glyph.width + PADDING;
            row_height = row_height.max(glyph.height);
        }

        // Create the glyph atlas.
        let used_height = y + row_height + PADDING;
        let atlas_height = used_height.next_power_of_two();
        let mut atlas_data = vec![0; ATLAS_WIDTH * atlas_height];
        let mut glyphs = HashMap::new();
        for glyph in rasterized {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }

            for row in 0..glyph.height {
                let src_start = row * glyph.width;
                let src_end = src_start + glyph.width;
                let dst_start = (glyph.y + row) * ATLAS_WIDTH + glyph.x;
                let dst_end = dst_start + glyph.width;
                atlas_data[dst_start..dst_end].copy_from_slice(&glyph.data[src_start..src_end]);
            }

            glyphs.insert(
                glyph.character,
                Glyph {
                    uv_min: [
                        glyph.x as f32 / ATLAS_WIDTH as f32,
                        glyph.y as f32 / atlas_height as f32,
                    ],
                    uv_max: [
                        (glyph.x + glyph.width) as f32 / ATLAS_WIDTH as f32,
                        (glyph.y + glyph.height) as f32 / atlas_height as f32,
                    ],
                },
            );
        }

        Ok(Self {
            font,
            size,
            atlas_width: ATLAS_WIDTH as u32,
            atlas_height: atlas_height as u32,
            atlas_data,
            glyphs,
        })
    }
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => {
                write!(f, "IO Error: {}", err)
            }
            Self::Font(err) => {
                write!(f, "Failed to load the font: {}", err)
            }
        }
    }
}
impl std::error::Error for TextError {}
mod error {
    use super::TextError;
    impl From<std::io::Error> for TextError {
        fn from(value: std::io::Error) -> Self {
            Self::Io(value.to_string())
        }
    }
    impl From<&str> for TextError {
        fn from(value: &str) -> Self {
            Self::Font(value.to_string())
        }
    }
}
