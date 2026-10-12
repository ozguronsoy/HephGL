use crate::math::{Mat3, Mat4, MathError, Quat, Vec3, Vec4, orthographic, perspective};

/// Defines how a camera projects view-space positions into clip space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    /// Makes distant objects appear smaller.
    Perspective {
        /// The vertical field of view in radians, between `0` and pi, exclusive.
        vertical_fov: f32,
        /// The viewport width divided by its height. Must be positive.
        aspect_ratio: f32,
        /// The distance to the near clipping plane. Must be positive.
        near: f32,
        /// The distance to the far clipping plane. Must be greater than `near`.
        far: f32,
    },
    /// Preserves an object's apparent size regardless of its distance from the camera.
    Orthographic {
        /// The left clipping plane in view space.
        left: f32,
        /// The right clipping plane in view space. Must be greater than `left`.
        right: f32,
        /// The bottom clipping plane in view space.
        bottom: f32,
        /// The top clipping plane in view space. Must be greater than `bottom`.
        top: f32,
        /// The signed distance to the near clipping plane along the camera's forward axis.
        near: f32,
        /// The signed distance to the far clipping plane. Must be greater than `near`.
        far: f32,
    },
}

/// Represents a camera's world-space position, orientation, and projection.
///
/// Uses left-handed coordinates with positive X pointing right, positive Y pointing up, and
/// positive Z pointing forward. The default camera is at the origin with no rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// The position of the camera in world space.
    pub position: Vec3,
    /// The normalized rotation from camera space to world space.
    pub rotation: Quat,
    /// The projection used by the camera.
    pub projection: Projection,
}

impl Camera {
    /// Creates a camera at the origin with the provided projection and no rotation.
    pub const fn new(projection: Projection) -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            projection,
        }
    }

    /// Rotates the camera to face a world-space target using the provided world-space up direction.
    pub fn look_at(&mut self, target: &Vec3, up: &Vec3) -> Result<(), MathError> {
        let forward = (target - self.position).try_normalize().ok_or_else(|| {
            MathError::invalid_argument("Camera target direction must be finite and nonzero.")
        })?;
        let up = up.try_normalize().ok_or_else(|| {
            MathError::invalid_argument("Camera up direction must be finite and nonzero.")
        })?;
        let right = up.cross(forward).try_normalize().ok_or_else(|| {
            MathError::invalid_argument(
                "Camera up direction must not be parallel to its target direction.",
            )
        })?;
        let up = forward.cross(right).try_normalize().ok_or_else(|| {
            MathError::invalid_argument("Camera directions must form a normalized basis.")
        })?;
        if !right.is_normalized() || !up.is_normalized() || !forward.is_normalized() {
            return Err(MathError::invalid_argument(
                "Camera directions must form a normalized basis.",
            ));
        }
        let rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
        let rotation = Vec4::from(rotation).try_normalize().ok_or_else(|| {
            MathError::invalid_argument("Camera directions must produce a normalized rotation.")
        })?;
        self.rotation = Quat::from_vec4(rotation);
        Ok(())
    }

    /// Calculates the matrix that transforms world-space positions into camera space.
    pub fn view_matrix(&self) -> Result<Mat4, MathError> {
        if !self.position.is_finite() {
            return Err(MathError::invalid_argument(
                "Camera position must be finite.",
            ));
        }
        self.validate_rotation()?;
        let inverse_rotation = self.rotation.conjugate();
        let matrix =
            Mat4::from_rotation_translation(inverse_rotation, inverse_rotation * -self.position);
        if !matrix.is_finite() {
            return Err(MathError::invalid_argument(
                "Camera parameters must produce a finite view matrix.",
            ));
        }
        Ok(matrix)
    }

    /// Calculates the camera's projection matrix. See `Projection::matrix` for parameter
    /// constraints.
    pub fn projection_matrix(&self) -> Result<Mat4, MathError> {
        self.projection.matrix()
    }

    /// Calculates `projection * view`, transforming world-space positions into clip space.
    pub fn view_projection_matrix(&self) -> Result<Mat4, MathError> {
        let matrix = self.projection_matrix()? * self.view_matrix()?;
        if !matrix.is_finite() {
            return Err(MathError::invalid_argument(
                "Camera parameters must produce a finite view-projection matrix.",
            ));
        }
        Ok(matrix)
    }

    /// Gets the camera's forward direction in world space.
    pub fn forward(&self) -> Result<Vec3, MathError> {
        self.validate_rotation()?;
        Ok(self.rotation * Vec3::Z)
    }

    /// Gets the camera's right direction in world space.
    pub fn right(&self) -> Result<Vec3, MathError> {
        self.validate_rotation()?;
        Ok(self.rotation * Vec3::X)
    }

    /// Gets the camera's up direction in world space.
    pub fn up(&self) -> Result<Vec3, MathError> {
        self.validate_rotation()?;
        Ok(self.rotation * Vec3::Y)
    }

    fn validate_rotation(&self) -> Result<(), MathError> {
        if !self.rotation.is_normalized() {
            return Err(MathError::invalid_argument(
                "Camera rotation must be normalized.",
            ));
        }
        Ok(())
    }
}
impl Default for Camera {
    fn default() -> Self {
        Self::new(Projection::default())
    }
}

impl Projection {
    /// Calculates the projection matrix for the renderer's Vulkan clip-space convention.
    pub fn matrix(&self) -> Result<Mat4, MathError> {
        let mut matrix = match *self {
            Self::Perspective {
                vertical_fov,
                aspect_ratio,
                near,
                far,
            } => {
                if !vertical_fov.is_finite()
                    || vertical_fov <= 0.0
                    || vertical_fov >= std::f32::consts::PI
                {
                    return Err(MathError::invalid_argument(
                        "Vertical field of view must be finite and between 0 and pi radians.",
                    ));
                }
                if !aspect_ratio.is_finite() || aspect_ratio <= 0.0 {
                    return Err(MathError::invalid_argument(
                        "Aspect ratio must be finite and positive.",
                    ));
                }
                if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
                    return Err(MathError::invalid_argument(
                        "Perspective clipping planes must be finite and satisfy 0 < near < far.",
                    ));
                }
                perspective(vertical_fov, aspect_ratio, near, far)
            }
            Self::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => {
                if !left.is_finite()
                    || !right.is_finite()
                    || !bottom.is_finite()
                    || !top.is_finite()
                    || right <= left
                    || top <= bottom
                {
                    return Err(MathError::invalid_argument(
                        "Orthographic bounds must be finite and satisfy left < right and bottom < top.",
                    ));
                }
                if !near.is_finite() || !far.is_finite() || far <= near {
                    return Err(MathError::invalid_argument(
                        "Orthographic clipping planes must be finite and satisfy near < far.",
                    ));
                }
                orthographic(left, right, bottom, top, near, far)
            }
        };

        // Flip the entire Y row, including the translation in off-center orthographic projections.
        matrix.x_axis.y = -matrix.x_axis.y;
        matrix.y_axis.y = -matrix.y_axis.y;
        matrix.z_axis.y = -matrix.z_axis.y;
        matrix.w_axis.y = -matrix.w_axis.y;
        if !matrix.is_finite() {
            return Err(MathError::invalid_argument(
                "Projection parameters must produce a finite matrix.",
            ));
        }
        Ok(matrix)
    }
}
impl Default for Projection {
    fn default() -> Self {
        Self::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_3,
            aspect_ratio: 1.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}
