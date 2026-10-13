use std::{collections::HashMap, path::Path};

pub use error::TextError;
use fontdue::{
    FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};
use glyph::{Glyph, RasterizedGlyph};

use crate::{
    math::Vec3,
    text::options::{FontOptions, TextType},
};

pub mod error;
mod glyph;
mod mesh;
pub mod options;

/// Represents a text vertex containing a 3D position, a normal, texture coordinates, and an RGBA
/// color, in that order.
pub type TextVertex = [f32; 12];

/// Represents a loaded font.
pub struct Font {
    /// The parsed font data used for layout and glyph rasterization.
    font: fontdue::Font,
    /// The original font data used to read glyph outlines.
    data: Vec<u8>,
    /// The options font is created with.
    options: FontOptions,
    /// The width of the glyph atlas in pixels.
    atlas_width: u32,
    /// The height of the glyph atlas in pixels.
    atlas_height: u32,
    /// The single-channel pixel data of the glyph atlas.
    atlas_data: Vec<u8>,
    /// The atlas metadata for each rasterized glyph.
    glyphs: HashMap<char, Glyph>,
    /// The last rasterized glyph.
    last_glyph: Option<char>,
    /// The height of the last row of the glyph atlas in pixels.
    last_row_height: usize,
}

impl Font {
    const ATLAS_WIDTH: usize = 1024;
    const PADDING: usize = 1;

    /// Loads a font from a `.ttf` or `.otf` file at a positive size in pixels.
    pub fn from_file<P: AsRef<Path>>(path: P, options: &FontOptions) -> Result<Self, TextError> {
        let data = std::fs::read(path)?;
        Self::new(data, options)
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

    /// Calculates triangle-list vertices of the provided text, starting at the top-left position.
    /// Screen text uses pixel coordinates and a Z value between zero and one. World and extruded
    /// text use local coordinates with positive Y up, ready for a model and camera matrix.
    ///
    /// Screen and world text sample the font atlas. Generate their vertices before uploading the
    /// atlas; adding new characters can resize it and invalidate previously generated UVs.
    /// Extruded text uses solid geometry and does not require an atlas texture.
    pub fn vertices(
        &mut self,
        text: &str,
        position: &Vec3,
        color: [f32; 4],
        text_type: TextType,
    ) -> Result<Vec<TextVertex>, TextError> {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings::default());
        layout.append(&[&self.font], &TextStyle::new(text, self.options.size, 0));

        match text_type {
            TextType::Screen { viewport_size } => {
                self.screen_vertices(text, &layout, position, color, viewport_size)
            }
            TextType::World { scale } => self.world_vertices(text, &layout, position, color, scale),
            TextType::Extruded { scale, depth } => {
                self.extruded_vertices(&layout, position, color, scale, depth)
            }
        }
    }

    fn new(font_data: Vec<u8>, options: &FontOptions) -> Result<Self, TextError> {
        if options.size <= 0.0 {
            return Err(TextError::invalid_argument("Font size must be positive."));
        }

        let font = fontdue::Font::from_bytes(font_data.as_slice(), FontSettings::default())?;
        let mut new_font = Self {
            font,
            data: match options.enable_extruded_text {
                true => font_data,
                false => Vec::new(),
            },
            options: *options,
            atlas_width: Self::ATLAS_WIDTH as u32,
            atlas_height: 0,
            atlas_data: Vec::new(),
            glyphs: HashMap::new(),
            last_glyph: None,
            last_row_height: 0,
        };

        if new_font.atlas_enabled() {
            let mut x = Self::PADDING;
            let mut y = Self::PADDING;
            let mut row_height = 0;
            let rasterized_glyphs = match options.load_all_characters {
                true => {
                    let mut characters: Vec<char> = new_font.font.chars().keys().copied().collect();
                    // Sorting the characters ensures that the atlas generated is deterministic and
                    // consistent across different platforms and runs.
                    characters.sort_unstable();
                    new_font.create_rasterized_glyphs(characters, &mut x, &mut y, &mut row_height)
                }
                false => {
                    // Rasterize only ASCII characters since they are the most commonly used. Other
                    // unicode characters will be rasterized lazily.
                    new_font.create_rasterized_glyphs(
                        '\x20'..='\x7E',
                        &mut x,
                        &mut y,
                        &mut row_height,
                    )
                }
            }?;

            let used_height = y + row_height + Self::PADDING;
            new_font.atlas_height = used_height.next_power_of_two() as u32;
            new_font.last_row_height = row_height;
            new_font
                .atlas_data
                .resize((new_font.atlas_width * new_font.atlas_height) as usize, 0);
            new_font.fill_atlas(&rasterized_glyphs);
        }

        Ok(new_font)
    }

    fn create_rasterized_glyphs<T>(
        &self,
        characters: T,
        x: &mut usize,
        y: &mut usize,
        row_height: &mut usize,
    ) -> Result<Vec<RasterizedGlyph>, TextError>
    where
        T: IntoIterator<Item = char>,
    {
        let mut rasterized_glyphs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for character in characters {
            if !seen.insert(character) || self.glyphs.contains_key(&character) {
                continue;
            }

            let metrics = self.font.metrics(character, self.options.size);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            if metrics.width > Self::ATLAS_WIDTH - Self::PADDING * 2 {
                return Err(TextError::invalid_argument(
                    "Font glyphs must fit within the atlas width.",
                ));
            }
            let data = self.font.rasterize(character, self.options.size).1;

            if *x + metrics.width + Self::PADDING > Self::ATLAS_WIDTH {
                // Current row is full, add a new row.
                *x = Self::PADDING;
                *y += *row_height + Self::PADDING;
                *row_height = 0;
            }

            rasterized_glyphs.push(RasterizedGlyph {
                character,
                data,
                glyph: Glyph {
                    x: *x,
                    y: *y,
                    width: metrics.width,
                    height: metrics.height,
                },
            });

            *x += metrics.width + Self::PADDING;
            *row_height = (*row_height).max(metrics.height);
        }
        Ok(rasterized_glyphs)
    }

    /// Adds missing characters to the glyph atlas.
    fn expand_atlas(&mut self, text: &str) -> Result<(), TextError> {
        let last_glyph = match self.last_glyph {
            Some(last_glyph) => &self.glyphs[&last_glyph],
            None => &Glyph {
                x: Self::PADDING,
                y: Self::PADDING,
                width: 0,
                height: 0,
            },
        };
        let mut x = last_glyph.x + last_glyph.width + Self::PADDING;
        let mut y = last_glyph.y;
        let mut row_height = self.last_row_height;
        let rasterized_glyphs =
            self.create_rasterized_glyphs(text.chars(), &mut x, &mut y, &mut row_height)?;

        let used_height = y + row_height + Self::PADDING;
        let atlas_height = used_height.next_power_of_two();
        let atlas_width = self.atlas_width as usize;
        if atlas_height > self.atlas_height as usize {
            self.atlas_height = atlas_height as u32;
            self.atlas_data.resize(atlas_width * atlas_height, 0);
        }
        self.last_row_height = row_height;
        self.fill_atlas(&rasterized_glyphs);
        Ok(())
    }

    fn fill_atlas(&mut self, glyphs: &[RasterizedGlyph]) {
        for rasterized_glyph in glyphs {
            let glyph = &rasterized_glyph.glyph;
            if self.glyphs.contains_key(&rasterized_glyph.character)
                || glyph.width == 0
                || glyph.height == 0
            {
                continue;
            }

            for row in 0..glyph.height {
                let src_start = row * glyph.width;
                let src_end = src_start + glyph.width;
                let dst_start = (glyph.y + row) * (self.atlas_width as usize) + glyph.x;
                let dst_end = dst_start + glyph.width;
                self.atlas_data[dst_start..dst_end]
                    .copy_from_slice(&rasterized_glyph.data[src_start..src_end]);
            }

            self.glyphs.insert(rasterized_glyph.character, *glyph);
            self.last_glyph = Some(rasterized_glyph.character);
        }
    }

    fn screen_vertices(
        &mut self,
        text: &str,
        layout: &Layout,
        position: &Vec3,
        color: [f32; 4],
        viewport_size: [f32; 2],
    ) -> Result<Vec<TextVertex>, TextError> {
        if !self.options.enable_screen_text {
            return Err(TextError::invalid_argument("Screen text is disabled."));
        }
        if viewport_size[0] <= 0.0
            || viewport_size[1] <= 0.0
            || position.z < 0.0
            || position.z > 1.0
        {
            return Err(TextError::invalid_argument(
                "Screen text requires a positive viewport and a depth between zero and one.",
            ));
        }
        self.quad_vertices(text, layout, color, |x, y| {
            [
                (position.x + x) / viewport_size[0] * 2.0 - 1.0,
                (position.y + y) / viewport_size[1] * 2.0 - 1.0,
                position.z,
            ]
        })
    }

    fn world_vertices(
        &mut self,
        text: &str,
        layout: &Layout,
        position: &Vec3,
        color: [f32; 4],
        scale: f32,
    ) -> Result<Vec<TextVertex>, TextError> {
        if !self.options.enable_world_text {
            return Err(TextError::invalid_argument("World text is disabled."));
        }
        if scale <= 0.0 {
            return Err(TextError::invalid_argument("Text scale must be positive."));
        }
        self.quad_vertices(text, layout, color, |x, y| {
            [position.x + x * scale, position.y - y * scale, position.z]
        })
    }

    fn extruded_vertices(
        &self,
        layout: &Layout,
        position: &Vec3,
        color: [f32; 4],
        scale: f32,
        depth: f32,
    ) -> Result<Vec<TextVertex>, TextError> {
        if !self.options.enable_extruded_text {
            return Err(TextError::invalid_argument("Extruded text is disabled."));
        }
        if scale <= 0.0 {
            return Err(TextError::invalid_argument("Text scale must be positive."));
        }
        if depth <= 0.0 {
            return Err(TextError::invalid_argument("Text depth must be positive."));
        }
        let face = ttf_parser::Face::parse(&self.data, 0)?;
        let mut vertices = Vec::with_capacity(layout.glyphs().len() * 6);
        for positioned in layout.glyphs() {
            if positioned.width == 0 || positioned.height == 0 {
                continue;
            }

            let metrics = self
                .font
                .metrics_indexed(positioned.key.glyph_index, self.options.size);
            let offset = Vec3::new(
                position.x + (positioned.x - metrics.xmin as f32) * scale,
                position.y
                    - (positioned.y + (metrics.bounds.height + metrics.bounds.ymin).ceil()) * scale,
                position.z,
            );
            vertices.extend(mesh::extrude(
                &face,
                positioned.key.glyph_index,
                self.options.size / face.units_per_em() as f32 * scale,
                depth,
                &offset,
                color,
            )?);
        }
        Ok(vertices)
    }

    fn quad_vertices(
        &mut self,
        text: &str,
        layout: &Layout,
        color: [f32; 4],
        transform_position: impl Fn(f32, f32) -> [f32; 3],
    ) -> Result<Vec<TextVertex>, TextError> {
        self.expand_atlas(text)?;
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

            let x0 = positioned.x;
            let y0 = positioned.y;
            let x1 = x0 + positioned.width as f32;
            let y1 = y0 + positioned.height as f32;

            let u0 = glyph.x as f32 / self.atlas_width as f32;
            let v0 = glyph.y as f32 / self.atlas_height as f32;
            let u1 = (glyph.x + glyph.width) as f32 / self.atlas_width as f32;
            let v1 = (glyph.y + glyph.height) as f32 / self.atlas_height as f32;

            let vertex = |x, y, u, v| {
                let [x, y, z] = transform_position(x, y);
                [
                    x, y, z, 0.0, 0.0, -1.0, u, v, color[0], color[1], color[2], color[3],
                ]
            };
            vertices.extend_from_slice(&[
                vertex(x0, y0, u0, v0),
                vertex(x1, y0, u1, v0),
                vertex(x1, y1, u1, v1),
                vertex(x0, y0, u0, v0),
                vertex(x1, y1, u1, v1),
                vertex(x0, y1, u0, v1),
            ]);
        }

        Ok(vertices)
    }

    fn atlas_enabled(&self) -> bool {
        self.options.enable_screen_text || self.options.enable_world_text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] =
        include_bytes!("../../assets/liberation_sans/LiberationSans-Regular.ttf");

    #[test]
    fn test_screen_and_world_coordinates() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_screen_text: true,
                enable_world_text: true,
                ..Default::default()
            },
        )
        .unwrap();
        let viewport_size = [800.0, 600.0];
        let screen_position = Vec3::new(80.0, 40.0, 0.35);
        let world_position = Vec3::new(-3.0, 5.0, 7.0);
        let scale = 0.025;
        let color = [0.2, 0.4, 0.6, 0.8];
        let screen = font
            .vertices(
                "Aé\nB",
                &screen_position,
                color,
                TextType::Screen { viewport_size },
            )
            .unwrap();
        let world = font
            .vertices("Aé\nB", &world_position, color, TextType::World { scale })
            .unwrap();

        assert_eq!(screen.len(), 18);
        assert_eq!(world.len(), screen.len());
        assert!(screen[2][1] > screen[0][1]);
        assert!(world[2][1] < world[0][1]);
        for (screen, world) in screen.iter().zip(&world) {
            let x = (screen[0] + 1.0) * viewport_size[0] / 2.0 - screen_position.x;
            let y = (screen[1] + 1.0) * viewport_size[1] / 2.0 - screen_position.y;
            assert!((world[0] - (world_position.x + x * scale)).abs() < 1e-5);
            assert!((world[1] - (world_position.y - y * scale)).abs() < 1e-5);
            assert_eq!(screen[2], screen_position.z);
            assert_eq!(world[2], world_position.z);
            assert_eq!(&screen[3..6], &[0.0, 0.0, -1.0]);
            assert_eq!(&world[3..], &screen[3..]);
            assert!((0.0..=1.0).contains(&screen[6]));
            assert!((0.0..=1.0).contains(&screen[7]));
            assert_eq!(&screen[8..], &color);
        }
    }

    #[test]
    fn test_world_translation_and_scale() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_world_text: true,
                ..Default::default()
            },
        )
        .unwrap();
        let position = Vec3::new(4.0, -3.0, 8.0);
        let original = font
            .vertices(
                "Åβ\njg",
                &Vec3::ZERO,
                [1.0; 4],
                TextType::World { scale: 0.025 },
            )
            .unwrap();
        let transformed = font
            .vertices(
                "Åβ\njg",
                &position,
                [1.0; 4],
                TextType::World { scale: 0.05 },
            )
            .unwrap();

        assert_eq!(original.len(), 24);
        assert_eq!(transformed.len(), original.len());
        for (original, transformed) in original.iter().zip(&transformed) {
            let original_position = Vec3::from_slice(&original[..3]);
            let transformed_position = Vec3::from_slice(&transformed[..3]);
            assert!(transformed_position.abs_diff_eq(original_position * 2.0 + position, 1e-6));
            assert_eq!(&original[3..], &transformed[3..]);
        }
    }

    #[test]
    fn test_multiline_text() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_screen_text: true,
                enable_world_text: true,
                enable_extruded_text: true,
                ..Default::default()
            },
        )
        .unwrap();
        for text_type in [
            TextType::Screen {
                viewport_size: [800.0, 600.0],
            },
            TextType::World { scale: 0.025 },
            TextType::Extruded {
                scale: 0.025,
                depth: 0.2,
            },
        ] {
            let single = font
                .vertices("A", &Vec3::ZERO, [1.0; 4], text_type)
                .unwrap();
            let multiline = font
                .vertices("A\nA", &Vec3::ZERO, [1.0; 4], text_type)
                .unwrap();
            assert!(!single.is_empty());
            assert_eq!(multiline.len(), single.len() * 2);
            assert_eq!(&multiline[..single.len()], single.as_slice());

            let line_offset = multiline[single.len()][1] - single[0][1];
            match text_type {
                TextType::Screen { .. } => assert!(line_offset > 0.0),
                _ => assert!(line_offset < 0.0),
            }
            for (first, second) in single.iter().zip(&multiline[single.len()..]) {
                assert!((first[0] - second[0]).abs() < 1e-6);
                assert!((first[1] + line_offset - second[1]).abs() < 1e-6);
                assert_eq!(&first[2..], &second[2..]);
            }
        }
    }

    #[test]
    fn test_extruded_glyph_bounds_and_depth() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_world_text: true,
                enable_extruded_text: true,
                ..Default::default()
            },
        )
        .unwrap();
        let position = Vec3::new(-2.0, 3.0, 5.0);
        let scale = 0.03125;
        let depth = 0.4;
        let color = [0.2, 0.4, 0.6, 0.8];
        for text in ["A", "g", "é", "Å", "j"] {
            let planar = font
                .vertices(text, &position, color, TextType::World { scale })
                .unwrap();
            let extruded = font
                .vertices(text, &position, color, TextType::Extruded { scale, depth })
                .unwrap();
            assert_eq!(planar.len(), 6);
            assert!(extruded.len() > 12);
            assert_eq!(extruded.len() % 3, 0);

            let minimum = Vec3::new(planar[0][0], planar[2][1], position.z);
            let maximum = Vec3::new(planar[2][0], planar[0][1], position.z + depth);
            let mut actual_minimum = Vec3::splat(f32::INFINITY);
            let mut actual_maximum = Vec3::splat(f32::NEG_INFINITY);
            let mut has_front = false;
            let mut has_back = false;
            let mut has_side = false;
            for vertex in &extruded {
                let point = Vec3::from_slice(&vertex[..3]);
                assert!(
                    point.cmpge(minimum - Vec3::splat(1e-5)).all(),
                    "{text}: {point:?}"
                );
                assert!(
                    point.cmple(maximum + Vec3::splat(1e-5)).all(),
                    "{text}: {point:?}"
                );
                actual_minimum = actual_minimum.min(point);
                actual_maximum = actual_maximum.max(point);
                let normal = Vec3::from_slice(&vertex[3..6]);
                assert!((normal.length() - 1.0).abs() < 1e-5);
                assert_eq!(&vertex[8..], &color);
                has_front |= normal == -Vec3::Z;
                has_back |= normal == Vec3::Z;
                has_side |= normal.z == 0.0;
            }
            assert!(actual_minimum.abs_diff_eq(minimum, scale + 1e-5), "{text}");
            assert!(actual_maximum.abs_diff_eq(maximum, scale + 1e-5), "{text}");
            assert_eq!(actual_minimum.z, position.z);
            assert_eq!(actual_maximum.z, position.z + depth);
            assert!(has_front && has_back && has_side);
        }
    }

    #[test]
    fn test_empty_text() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_screen_text: true,
                enable_world_text: true,
                enable_extruded_text: true,
                ..Default::default()
            },
        )
        .unwrap();
        for text_type in [
            TextType::Screen {
                viewport_size: [800.0, 600.0],
            },
            TextType::World { scale: 0.025 },
            TextType::Extruded {
                scale: 0.025,
                depth: 0.2,
            },
        ] {
            for text in ["", " \t\r\n "] {
                assert!(
                    font.vertices(text, &Vec3::ZERO, [1.0; 4], text_type)
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn test_disabled_text_types() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_screen_text: false,
                enable_world_text: false,
                enable_extruded_text: false,
                ..Default::default()
            },
        )
        .unwrap();
        for text_type in [
            TextType::Screen {
                viewport_size: [800.0, 600.0],
            },
            TextType::World { scale: 0.025 },
            TextType::Extruded {
                scale: 0.025,
                depth: 0.2,
            },
        ] {
            assert!(matches!(
                font.vertices("A", &Vec3::ZERO, [1.0; 4], text_type),
                Err(TextError::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn test_invalid_text_parameters() {
        let mut font = Font::new(
            FONT_DATA.to_vec(),
            &FontOptions {
                enable_screen_text: true,
                enable_world_text: true,
                enable_extruded_text: true,
                ..Default::default()
            },
        )
        .unwrap();
        for value in [0.0, -1.0] {
            for text_type in [
                TextType::Screen {
                    viewport_size: [value, 600.0],
                },
                TextType::Screen {
                    viewport_size: [800.0, value],
                },
                TextType::World { scale: value },
                TextType::Extruded {
                    scale: value,
                    depth: 0.2,
                },
                TextType::Extruded {
                    scale: 0.025,
                    depth: value,
                },
            ] {
                assert!(matches!(
                    font.vertices("A", &Vec3::ZERO, [1.0; 4], text_type),
                    Err(TextError::InvalidArgument(_))
                ));
            }
        }
        for depth in [-0.1, 1.1] {
            assert!(matches!(
                font.vertices(
                    "A",
                    &Vec3::new(0.0, 0.0, depth),
                    [1.0; 4],
                    TextType::Screen {
                        viewport_size: [800.0, 600.0]
                    },
                ),
                Err(TextError::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn test_invalid_font_size_and_data() {
        for size in [0.0, -1.0, 1_000_000.0] {
            assert!(matches!(
                Font::new(
                    FONT_DATA.to_vec(),
                    &FontOptions {
                        size,
                        ..FontOptions::default()
                    }
                ),
                Err(TextError::InvalidArgument(_))
            ));
        }
        assert!(matches!(
            Font::new(Vec::new(), &FontOptions::default()),
            Err(TextError::Font(_))
        ));
    }
}
