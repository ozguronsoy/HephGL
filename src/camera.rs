use crate::{
    Error, Result,
    math::{Mat3, Mat4, Quat, Vec3, Vec4, orthographic, perspective},
};

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
    pub fn look_at(&mut self, target: &Vec3, up: &Vec3) -> Result<()> {
        let forward = (target - self.position).try_normalize().ok_or_else(|| {
            Error::invalid_argument("Camera target direction must be finite and nonzero.")
        })?;
        let up = up.try_normalize().ok_or_else(|| {
            Error::invalid_argument("Camera up direction must be finite and nonzero.")
        })?;
        let right = up.cross(forward).try_normalize().ok_or_else(|| {
            Error::invalid_argument(
                "Camera up direction must not be parallel to its target direction.",
            )
        })?;
        let up = forward.cross(right).try_normalize().ok_or_else(|| {
            Error::invalid_argument("Camera directions must form a normalized basis.")
        })?;
        if !right.is_normalized() || !up.is_normalized() || !forward.is_normalized() {
            return Err(Error::invalid_argument(
                "Camera directions must form a normalized basis.",
            ));
        }
        let rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
        let rotation = Vec4::from(rotation).try_normalize().ok_or_else(|| {
            Error::invalid_argument("Camera directions must produce a normalized rotation.")
        })?;
        self.rotation = Quat::from_vec4(rotation);
        Ok(())
    }

    /// Calculates the matrix that transforms world-space positions into camera space.
    pub fn view_matrix(&self) -> Result<Mat4> {
        if !self.position.is_finite() {
            return Err(Error::invalid_argument("Camera position must be finite."));
        }
        self.validate_rotation()?;
        let inverse_rotation = self.rotation.conjugate();
        let matrix =
            Mat4::from_rotation_translation(inverse_rotation, inverse_rotation * -self.position);
        if !matrix.is_finite() {
            return Err(Error::invalid_argument(
                "Camera parameters must produce a finite view matrix.",
            ));
        }
        Ok(matrix)
    }

    /// Calculates the camera's projection matrix. See `Projection::matrix` for parameter
    /// constraints.
    pub fn projection_matrix(&self) -> Result<Mat4> {
        self.projection.matrix()
    }

    /// Calculates `projection * view`, transforming world-space positions into clip space.
    pub fn view_projection_matrix(&self) -> Result<Mat4> {
        let matrix = self.projection_matrix()? * self.view_matrix()?;
        if !matrix.is_finite() {
            return Err(Error::invalid_argument(
                "Camera parameters must produce a finite view-projection matrix.",
            ));
        }
        Ok(matrix)
    }

    /// Gets the camera's forward direction in world space.
    pub fn forward(&self) -> Result<Vec3> {
        self.validate_rotation()?;
        Ok(self.rotation * Vec3::Z)
    }

    /// Gets the camera's right direction in world space.
    pub fn right(&self) -> Result<Vec3> {
        self.validate_rotation()?;
        Ok(self.rotation * Vec3::X)
    }

    /// Gets the camera's up direction in world space.
    pub fn up(&self) -> Result<Vec3> {
        self.validate_rotation()?;
        Ok(self.rotation * Vec3::Y)
    }

    fn validate_rotation(&self) -> Result<()> {
        if !self.rotation.is_normalized() {
            return Err(Error::invalid_argument(
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
    pub fn matrix(&self) -> Result<Mat4> {
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
                    return Err(Error::invalid_argument(
                        "Vertical field of view must be finite and between 0 and pi radians.",
                    ));
                }
                if !aspect_ratio.is_finite() || aspect_ratio <= 0.0 {
                    return Err(Error::invalid_argument(
                        "Aspect ratio must be finite and positive.",
                    ));
                }
                if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
                    return Err(Error::invalid_argument(
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
                    return Err(Error::invalid_argument(
                        "Orthographic bounds must be finite and satisfy left < right and bottom < top.",
                    ));
                }
                if !near.is_finite() || !far.is_finite() || far <= near {
                    return Err(Error::invalid_argument(
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
            return Err(Error::invalid_argument(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_default() {
        let projection = Projection::Orthographic {
            left: -2.0,
            right: 2.0,
            bottom: -1.0,
            top: 1.0,
            near: -5.0,
            far: 5.0,
        };
        let camera = Camera::new(projection);
        assert_eq!(camera.position, Vec3::ZERO);
        assert_eq!(camera.rotation, Quat::IDENTITY);
        assert_eq!(camera.projection, projection);
        assert_eq!(camera.view_matrix().unwrap(), Mat4::IDENTITY);
        assert_eq!(
            camera.projection_matrix().unwrap(),
            projection.matrix().unwrap()
        );

        let camera = Camera::default();
        assert_eq!(camera, Camera::new(Projection::default()));
        assert_eq!(
            camera.projection,
            Projection::Perspective {
                vertical_fov: std::f32::consts::FRAC_PI_3,
                aspect_ratio: 1.0,
                near: 0.1,
                far: 1000.0,
            }
        );
        assert_eq!(camera.view_matrix().unwrap(), Mat4::IDENTITY);
    }

    #[test]
    fn test_directions() {
        for (rotation, forward, right, up) in [
            (Quat::IDENTITY, Vec3::Z, Vec3::X, Vec3::Y),
            (
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                Vec3::X,
                -Vec3::Z,
                Vec3::Y,
            ),
            (
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                -Vec3::Y,
                Vec3::X,
                Vec3::Z,
            ),
        ] {
            let camera = Camera {
                position: Vec3::new(3.0, -2.0, 5.0),
                rotation,
                ..Camera::default()
            };
            assert!(camera.forward().unwrap().abs_diff_eq(forward, 1e-6));
            assert!(camera.right().unwrap().abs_diff_eq(right, 1e-6));
            assert!(camera.up().unwrap().abs_diff_eq(up, 1e-6));
        }
    }

    #[test]
    fn test_look_at_and_view_matrix() {
        let mut camera = Camera {
            position: Vec3::new(3.0, -2.0, 5.0),
            ..Camera::default()
        };
        let target = Vec3::new(7.0, 1.0, 5.0);
        camera.look_at(&target, &Vec3::new(0.0, 2.0, 0.0)).unwrap();

        assert!(camera.rotation.is_normalized());
        assert!(
            camera
                .forward()
                .unwrap()
                .abs_diff_eq(Vec3::new(0.8, 0.6, 0.0), 1e-6)
        );
        assert!(camera.right().unwrap().abs_diff_eq(-Vec3::Z, 1e-6));
        assert!(
            camera
                .up()
                .unwrap()
                .abs_diff_eq(Vec3::new(-0.6, 0.8, 0.0), 1e-6)
        );

        let view = camera.view_matrix().unwrap();
        assert!(
            view.transform_point3(camera.position)
                .abs_diff_eq(Vec3::ZERO, 1e-5)
        );
        assert!(
            view.transform_point3(target)
                .abs_diff_eq(Vec3::new(0.0, 0.0, 5.0), 1e-5)
        );
        assert!(
            view.transform_point3(Vec3::new(3.0, -2.0, 3.0))
                .abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), 1e-5)
        );

        let pose = Mat4::from_rotation_translation(camera.rotation, camera.position);
        assert!((view * pose).abs_diff_eq(Mat4::IDENTITY, 1e-5));
        assert!((pose * view).abs_diff_eq(Mat4::IDENTITY, 1e-5));
    }

    #[test]
    fn test_look_at_invalid_directions() {
        let mut camera = Camera {
            rotation: Quat::from_rotation_y(0.5),
            ..Camera::default()
        };
        let original = camera;
        for (target, up) in [
            (Vec3::ZERO, Vec3::Y),
            (Vec3::new(f32::NAN, 0.0, 1.0), Vec3::Y),
            (Vec3::new(0.0, f32::INFINITY, 1.0), Vec3::Y),
            (Vec3::new(0.0, 0.0, f32::MIN_POSITIVE), Vec3::Y),
            (Vec3::Z, Vec3::ZERO),
            (Vec3::Z, Vec3::new(0.0, f32::NAN, 0.0)),
            (Vec3::Z, Vec3::new(f32::NEG_INFINITY, 1.0, 0.0)),
            (Vec3::Z, Vec3::Z),
            (Vec3::Z, -Vec3::Z),
        ] {
            assert!(matches!(
                camera.look_at(&target, &up),
                Err(Error::InvalidArgument(_))
            ));
            assert_eq!(camera, original);
        }
    }

    #[test]
    fn test_perspective_matrix() {
        let matrix = Projection::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_2,
            aspect_ratio: 2.0,
            near: 1.0,
            far: 11.0,
        }
        .matrix()
        .unwrap();

        assert!(
            matrix
                .project_point3(Vec3::new(2.0, 1.0, 1.0))
                .abs_diff_eq(Vec3::new(1.0, -1.0, 0.0), 1e-6)
        );
        assert!(
            matrix
                .project_point3(Vec3::new(0.0, 0.0, 11.0))
                .abs_diff_eq(Vec3::Z, 1e-6)
        );
        assert!(
            matrix
                .project_point3(Vec3::new(1.0, 1.0, 2.0))
                .abs_diff_eq(Vec3::new(0.25, -0.5, 0.55), 1e-6)
        );
        assert!(
            matrix
                .project_point3(Vec3::new(1.0, 1.0, 4.0))
                .abs_diff_eq(Vec3::new(0.125, -0.25, 0.825), 1e-6)
        );
    }

    #[test]
    fn test_orthographic_matrix() {
        let matrix = Projection::Orthographic {
            left: -2.0,
            right: 6.0,
            bottom: -3.0,
            top: 5.0,
            near: -1.0,
            far: 9.0,
        }
        .matrix()
        .unwrap();

        assert!(
            matrix
                .project_point3(Vec3::new(-2.0, -3.0, -1.0))
                .abs_diff_eq(Vec3::new(-1.0, 1.0, 0.0), 1e-6)
        );
        assert!(
            matrix
                .project_point3(Vec3::new(6.0, 5.0, 9.0))
                .abs_diff_eq(Vec3::new(1.0, -1.0, 1.0), 1e-6)
        );
        assert!(
            matrix
                .project_point3(Vec3::new(2.0, 1.0, 4.0))
                .abs_diff_eq(Vec3::new(0.0, 0.0, 0.5), 1e-6)
        );
        assert_eq!(
            matrix.project_point3(Vec3::new(4.0, 3.0, -1.0)).truncate(),
            matrix.project_point3(Vec3::new(4.0, 3.0, 9.0)).truncate(),
        );
    }

    #[test]
    fn test_view_projection_matrix() {
        let camera = Camera {
            position: Vec3::new(3.0, 2.0, -1.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            projection: Projection::Perspective {
                vertical_fov: std::f32::consts::FRAC_PI_2,
                aspect_ratio: 2.0,
                near: 1.0,
                far: 11.0,
            },
        };
        let matrix = camera.view_projection_matrix().unwrap();
        assert!(
            matrix
                .project_point3(Vec3::new(7.0, 3.0, -3.0))
                .abs_diff_eq(Vec3::new(0.25, -0.25, 0.825), 1e-6)
        );
    }

    #[test]
    fn test_invalid_position() {
        for position in [
            Vec3::new(f32::NAN, 0.0, 0.0),
            Vec3::new(0.0, f32::INFINITY, 0.0),
            Vec3::new(0.0, 0.0, f32::NEG_INFINITY),
        ] {
            let camera = Camera {
                position,
                ..Camera::default()
            };
            assert!(matches!(
                camera.view_matrix(),
                Err(Error::InvalidArgument(_))
            ));
            assert!(matches!(
                camera.view_projection_matrix(),
                Err(Error::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn test_invalid_rotation() {
        for rotation in [
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            Quat::from_xyzw(0.0, 0.0, 0.0, 2.0),
            Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
            Quat::from_xyzw(0.0, f32::INFINITY, 0.0, 1.0),
        ] {
            let camera = Camera {
                rotation,
                ..Camera::default()
            };
            assert!(matches!(
                camera.view_matrix(),
                Err(Error::InvalidArgument(_))
            ));
            assert!(matches!(
                camera.view_projection_matrix(),
                Err(Error::InvalidArgument(_))
            ));
            assert!(matches!(camera.forward(), Err(Error::InvalidArgument(_))));
            assert!(matches!(camera.right(), Err(Error::InvalidArgument(_))));
            assert!(matches!(camera.up(), Err(Error::InvalidArgument(_))));
        }
    }

    #[test]
    fn test_invalid_perspective_parameters() {
        let valid = [std::f32::consts::FRAC_PI_2, 2.0, 1.0, 11.0];
        let check_invalid = |[vertical_fov, aspect_ratio, near, far]: [f32; 4]| {
            let projection = Projection::Perspective {
                vertical_fov,
                aspect_ratio,
                near,
                far,
            };
            assert!(matches!(
                projection.matrix(),
                Err(Error::InvalidArgument(_))
            ));
            let camera = Camera::new(projection);
            assert!(matches!(
                camera.projection_matrix(),
                Err(Error::InvalidArgument(_))
            ));
            assert!(matches!(
                camera.view_projection_matrix(),
                Err(Error::InvalidArgument(_))
            ));
        };

        for (index, value) in [
            (0, 0.0),
            (0, -1.0),
            (0, std::f32::consts::PI),
            (0, 4.0),
            (1, 0.0),
            (1, -1.0),
            (2, 0.0),
            (2, -1.0),
            (2, 11.0),
            (2, 12.0),
            (3, 1.0),
            (3, 0.5),
        ] {
            let mut parameters = valid;
            parameters[index] = value;
            check_invalid(parameters);
        }
        for index in 0..valid.len() {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut parameters = valid;
                parameters[index] = value;
                check_invalid(parameters);
            }
        }
    }

    #[test]
    fn test_invalid_orthographic_parameters() {
        let valid = [-2.0, 6.0, -3.0, 5.0, -1.0, 9.0];
        let check_invalid = |[left, right, bottom, top, near, far]: [f32; 6]| {
            let projection = Projection::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            };
            assert!(matches!(
                projection.matrix(),
                Err(Error::InvalidArgument(_))
            ));
        };

        for (index, value) in [
            (0, 6.0),
            (0, 7.0),
            (1, -2.0),
            (1, -3.0),
            (2, 5.0),
            (2, 6.0),
            (3, -3.0),
            (3, -4.0),
            (4, 9.0),
            (4, 10.0),
            (5, -1.0),
            (5, -2.0),
        ] {
            let mut parameters = valid;
            parameters[index] = value;
            check_invalid(parameters);
        }
        for index in 0..valid.len() {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut parameters = valid;
                parameters[index] = value;
                check_invalid(parameters);
            }
        }
    }

    #[test]
    fn test_matrix_overflow() {
        let camera = Camera {
            position: Vec3::new(f32::MAX, 0.0, f32::MAX),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
            ..Camera::default()
        };
        assert!(matches!(
            camera.view_matrix(),
            Err(Error::InvalidArgument(_))
        ));

        let camera = Camera {
            position: Vec3::new(0.0, f32::MAX, 0.0),
            ..Camera::default()
        };
        assert!(camera.view_matrix().unwrap().is_finite());
        assert!(camera.projection_matrix().unwrap().is_finite());
        assert!(matches!(
            camera.view_projection_matrix(),
            Err(Error::InvalidArgument(_))
        ));

        for projection in [
            Projection::Perspective {
                vertical_fov: f32::MIN_POSITIVE,
                aspect_ratio: 0.25,
                near: 1.0,
                far: 11.0,
            },
            Projection::Orthographic {
                left: 0.0,
                right: f32::MIN_POSITIVE * 0.25,
                bottom: -1.0,
                top: 1.0,
                near: 0.0,
                far: 1.0,
            },
        ] {
            assert!(matches!(
                projection.matrix(),
                Err(Error::InvalidArgument(_))
            ));
        }
    }
}
