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

struct RasterizedGlyph {
    character: char,
    width: usize,
    height: usize,
    data: Vec<u8>,
    x: usize,
    y: usize,
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
    const ATLAS_WIDTH: usize = 512;
    const PADDING: usize = 1;

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
        &mut self,
        text: &str,
        position: [f32; 2],
        viewport_size: [f32; 2],
        color: [f32; 4],
    ) -> Vec<TextVertex> {
        self.extend_atlas(text);

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
        let mut new_font = Self {
            font,
            size,
            atlas_width: Self::ATLAS_WIDTH as u32,
            atlas_height: 0,
            atlas_data: Vec::new(),
            glyphs: HashMap::new(),
        };

        let mut x = Self::PADDING;
        let mut y = Self::PADDING;
        let mut row_height = 0;
        // Rasterize only ASCII characters since they are the most commonly used. Other unicode
        // characters will be rasterized lazily.
        let rasterized_glyphs =
            new_font.create_rasterized_glyphs('\x20'..='\x7E', &mut x, &mut y, &mut row_height);

        let used_height = y + row_height + Self::PADDING;
        new_font.atlas_height = used_height.next_power_of_two() as u32;
        new_font
            .atlas_data
            .resize((new_font.atlas_width * new_font.atlas_height) as usize, 0);
        new_font.fill_atlas(&rasterized_glyphs);

        Ok(new_font)
    }

    fn create_rasterized_glyphs<T>(
        &self,
        characters: T,
        x: &mut usize,
        y: &mut usize,
        row_height: &mut usize,
    ) -> Vec<RasterizedGlyph>
    where
        T: IntoIterator<Item = char>,
    {
        let mut rasterized_glyphs = Vec::new();
        for character in characters {
            if self.glyphs.contains_key(&character) {
                continue;
            }

            let (metrics, data) = self.font.rasterize(character, self.size);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }

            if *x + metrics.width + Self::PADDING > Self::ATLAS_WIDTH {
                // Current row is full, add a new row.
                *x = Self::PADDING;
                *y += *row_height + Self::PADDING;
                *row_height = 0;
            }

            rasterized_glyphs.push(RasterizedGlyph {
                width: metrics.width,
                height: metrics.height,
                character,
                data,
                x: *x,
                y: *y,
            });

            *x += metrics.width + Self::PADDING;
            *row_height = (*row_height).max(metrics.height);
        }
        rasterized_glyphs
    }

    /// Adds missing characters to the glyph atlas.
    fn extend_atlas(&mut self, text: &str) {
        let last_glyph = self.glyphs.values().last().unwrap();
        let mut x = (last_glyph.uv_max[0] * self.atlas_width as f32) as usize + Self::PADDING;
        let mut y = (last_glyph.uv_min[1] * self.atlas_height as f32) as usize;
        let mut row_height = self.get_row_height(y);
        let rasterized_glyphs =
            self.create_rasterized_glyphs(text.chars(), &mut x, &mut y, &mut row_height);

        let used_height = y + row_height + Self::PADDING;
        let atlas_height = used_height.next_power_of_two();
        let atlas_width = self.atlas_width as usize;
        if atlas_height > self.atlas_height as usize {
            self.atlas_height = atlas_height as u32;
            self.atlas_data.resize(atlas_width * atlas_height, 0);
        }
        self.fill_atlas(&rasterized_glyphs);
    }

    fn fill_atlas(&mut self, glyphs: &[RasterizedGlyph]) {
        let atlas_width = self.atlas_width as usize;
        let atlas_height = self.atlas_height as usize;
        for glyph in glyphs {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }

            for row in 0..glyph.height {
                let src_start = row * glyph.width;
                let src_end = src_start + glyph.width;
                let dst_start = (glyph.y + row) * atlas_width + glyph.x;
                let dst_end = dst_start + glyph.width;
                self.atlas_data[dst_start..dst_end]
                    .copy_from_slice(&glyph.data[src_start..src_end]);
            }

            self.glyphs.insert(
                glyph.character,
                Glyph {
                    uv_min: [
                        glyph.x as f32 / atlas_width as f32,
                        glyph.y as f32 / atlas_height as f32,
                    ],
                    uv_max: [
                        (glyph.x + glyph.width) as f32 / Self::ATLAS_WIDTH as f32,
                        (glyph.y + glyph.height) as f32 / atlas_height as f32,
                    ],
                },
            );
        }
    }

    fn get_row_height(&self, y: usize) -> usize {
        let atlas_height = self.atlas_height as f32;
        let glyphs: Vec<&Glyph> = self.glyphs.values().collect();
        let mut row_height = 0;
        for &glyph in glyphs.iter().rev() {
            let gy_start = (glyph.uv_min[1] * atlas_height) as usize;
            let gy_end = (glyph.uv_max[1] * atlas_height) as usize;
            if gy_start > y {
                continue;
            }
            if gy_start < y {
                break;
            }

            let glyph_height = gy_end - gy_start;
            row_height = row_height.max(glyph_height)
        }
        row_height
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
