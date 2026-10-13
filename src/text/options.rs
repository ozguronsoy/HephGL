/// Defines the geometry and coordinate system used to render text.
#[derive(Clone, Copy, PartialEq)]
pub enum TextType {
    /// Generates textured quads in clip space from a position in pixels, with positive Y down.
    Screen {
        /// The viewport width and height in pixels. Both values must be positive.
        viewport_size: [f32; 2],
    },
    /// Generates textured quads in local space, with positive X right and positive Y up.
    World {
        /// The number of world units per font pixel. Must be positive.
        scale: f32,
    },
    /// Generates solid glyphs with front faces, back faces, and side walls in local space.
    Extruded {
        /// The number of world units per font pixel. Must be positive.
        scale: f32,
        /// The distance from the front face toward positive Z. Must be positive.
        depth: f32,
    },
}

/// Defines the options for loading a font and generating text meshes.
#[derive(Clone, Copy)]
pub struct FontOptions {
    /// The size of the font in pixels.
    pub size: f32,
    /// Indicates whether to load all unicode characters in the font. If `false`, only the ASCII
    /// characters will be loaded immediately, and other characters will be loaded lazily
    /// on-demand.
    pub load_all_characters: bool,
    /// Indicates whether to enable rendering of screen-space text.
    pub enable_screen_text: bool,
    /// Indicates whether to enable rendering of world-space text.
    pub enable_world_text: bool,
    /// Indicates whether to enable rendering of extruded text.
    pub enable_extruded_text: bool,
}

impl Default for FontOptions {
    fn default() -> Self {
        Self {
            size: 16.0,
            load_all_characters: false,
            enable_screen_text: true,
            enable_world_text: false,
            enable_extruded_text: false,
        }
    }
}
