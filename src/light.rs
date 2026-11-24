use crate::math::Vec3;

/// Defines directional and ambient lighting in world space.
#[derive(Clone, Copy)]
pub struct Lighting {
    /// The direction in which the light travels.
    pub direction: Vec3,
    /// The directional light's linear RGB color.
    pub color: [f32; 3],
    /// The directional light's intensity.
    pub intensity: f32,
    /// The ambient light's linear RGB color and intensity.
    pub ambient: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            direction: Vec3::ONE,
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            ambient: [1.0, 1.0, 1.0],
        }
    }
}
