use renkrs::RGB;

use crate::math::Vec3;

/// Defines directional and ambient lighting in world space.
#[derive(Clone, Copy)]
pub struct Lighting {
    /// The direction in which the light travels.
    pub direction: Vec3,
    /// The directional light's color in linear space.
    pub color: RGB<f32>,
    /// The directional light's intensity.
    pub intensity: f32,
    /// The ambient light's color and intensity in linear space.
    pub ambient: RGB<f32>,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            direction: Vec3::ONE,
            color: RGB::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            ambient: RGB::new(1.0, 1.0, 1.0),
        }
    }
}
