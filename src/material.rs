use renkrs::RGBA;

/// Describes the surface properties used to shade a rendered object.
#[derive(Clone, Copy)]
pub struct MaterialProperties {
    /// The linear RGBA tint multiplied with the base-color texture.
    pub base_color: RGBA<f32>,
    /// The roughness, between zero and one.
    pub roughness: f32,
    /// The metallicness, between zero and one. Higher values produce more metallic reflections.
    pub metalicness: f32,
    /// The nonnegative multiplier for the normal map's tangent-space X and Y components.
    pub normal_scale: f32,
    /// The displacement multiplier in local-space units. Positive values move vertices outward.
    pub displacement_scale: f32,
    /// The offset in local-space units added after multiplying the height by `displacement_scale`.
    pub displacement_bias: f32,
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            base_color: RGBA::new(1.0, 1.0, 1.0, 1.0),
            roughness: 1.0,
            metalicness: 0.0,
            normal_scale: 1.0,
            displacement_scale: 1.0,
            displacement_bias: 0.0,
        }
    }
}
