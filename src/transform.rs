use crate::{
    Error, Result,
    math::{Mat3, Mat4, Quat, Vec3},
};

/// Represents an object's translation, rotation, and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// The position relative to the parent coordinate system.
    pub translation: Vec3,
    /// The normalized rotation relative to the parent coordinate system.
    pub rotation: Quat,
    /// The scale along the local axes.
    pub scale: Vec3,
}

impl Transform {
    /// A transform with no translation, no rotation, and unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Creates a transform. The rotation must be normalized.
    pub const fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Calculates the model matrix, applying scale, rotation, then translation.
    pub fn matrix(&self) -> Result<Mat4> {
        self.validate_rotation_and_scale()?;
        if !self.translation.is_finite() {
            return Err(Error::invalid_argument(
                "Transform translation must be finite.",
            ));
        }
        let matrix =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        if !matrix.is_finite() {
            return Err(Error::invalid_argument("Transform matrix must be finite."));
        }
        Ok(matrix)
    }

    /// Calculates the inverse transpose of the model matrix's linear part for transforming normals.
    pub fn normal_matrix(&self) -> Result<Mat3> {
        self.validate_rotation_and_scale()?;
        let matrix = Mat3::from_quat(self.rotation) * Mat3::from_diagonal(self.scale.recip());
        if !matrix.is_finite() {
            return Err(Error::invalid_argument(
                "Transform linear matrix must have a finite inverse.",
            ));
        }
        Ok(matrix)
    }

    /// Transforms a local point into the parent coordinate system, including translation.
    pub fn transform_point(&self, point: &Vec3) -> Result<Vec3> {
        if !self.translation.is_finite() {
            return Err(Error::invalid_argument(
                "Transform translation must be finite.",
            ));
        }
        let point = self.transform_vector(point)? + self.translation;
        if !point.is_finite() {
            return Err(Error::invalid_argument("Transformed point must be finite."));
        }
        Ok(point)
    }

    /// Transforms a local vector into the parent coordinate system without translation.
    pub fn transform_vector(&self, vector: &Vec3) -> Result<Vec3> {
        self.validate_rotation_and_scale()?;
        if !vector.is_finite() {
            return Err(Error::invalid_argument("Transform input must be finite."));
        }
        let vector = self.rotation * (self.scale * vector);
        if !vector.is_finite() {
            return Err(Error::invalid_argument(
                "Transformed vector must be finite.",
            ));
        }
        Ok(vector)
    }

    fn validate_rotation_and_scale(&self) -> Result<()> {
        if !self.rotation.is_normalized() {
            return Err(Error::invalid_argument(
                "Transform rotation must be normalized.",
            ));
        }
        if !self.scale.is_finite() {
            return Err(Error::invalid_argument("Transform scale must be finite."));
        }
        Ok(())
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_new() {
        let translation = Vec3::new(1.0, 2.0, 3.0);
        let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let scale = Vec3::new(2.0, 3.0, 4.0);
        let transform = Transform::new(translation, rotation, scale);

        assert_eq!(transform.translation, translation);
        assert_eq!(transform.rotation, rotation);
        assert_eq!(transform.scale, scale);
    }

    #[test]
    fn test_transform_default() {
        let transform = Transform::default();
        let point = Vec3::new(1.0, 2.0, 3.0);

        assert_eq!(transform, Transform::IDENTITY);
        assert_eq!(transform.translation, Vec3::ZERO);
        assert_eq!(transform.rotation, Quat::IDENTITY);
        assert_eq!(transform.scale, Vec3::ONE);
        assert_eq!(transform.matrix().unwrap(), Mat4::IDENTITY);
        assert_eq!(transform.normal_matrix().unwrap(), Mat3::IDENTITY);
        assert_eq!(transform.transform_point(&point).unwrap(), point);
        assert_eq!(transform.transform_vector(&point).unwrap(), point);
    }

    #[test]
    fn test_transform_matrix() {
        let transform = Transform::new(
            Vec3::new(5.0, 7.0, 11.0),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::new(2.0, 3.0, 4.0),
        );
        let point = Vec3::new(1.0, 2.0, 3.0);
        let expected_point = Vec3::new(-1.0, 9.0, 23.0);
        let expected_vector = Vec3::new(-6.0, 2.0, 12.0);
        let expected_matrix = Mat4::from_cols_array(&[
            0.0, 2.0, 0.0, 0.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 5.0, 7.0, 11.0, 1.0,
        ]);
        let matrix = transform.matrix().unwrap();

        assert!(matrix.abs_diff_eq(expected_matrix, 1e-5));
        assert!(
            matrix
                .transform_point3(point)
                .abs_diff_eq(expected_point, 1e-5)
        );
        assert!(
            matrix
                .transform_vector3(point)
                .abs_diff_eq(expected_vector, 1e-5)
        );
        assert!(
            transform
                .transform_point(&point)
                .unwrap()
                .abs_diff_eq(expected_point, 1e-5)
        );
        assert!(
            transform
                .transform_vector(&point)
                .unwrap()
                .abs_diff_eq(expected_vector, 1e-5)
        );
    }

    #[test]
    fn test_transform_normal_matrix() {
        let transform = Transform::new(
            Vec3::new(5.0, 7.0, 11.0),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::new(-2.0, 3.0, 4.0),
        );
        let expected_matrix = Mat3::from_cols(
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(-1.0 / 3.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.25),
        );
        let matrix = transform.normal_matrix().unwrap();
        let normal = matrix * Vec3::ONE;
        let tangent = transform
            .transform_vector(&Vec3::new(1.0, -1.0, 0.0))
            .unwrap();

        assert!(matrix.abs_diff_eq(expected_matrix, 1e-6));
        assert!(normal.abs_diff_eq(Vec3::new(-1.0 / 3.0, -0.5, 0.25), 1e-6));
        assert!(normal.dot(tangent).abs() < 1e-6);
    }

    #[test]
    fn test_transform_normal_matrix_extreme_scales() {
        for scale in [1e-20, 1e20] {
            let transform = Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::splat(scale));
            let matrix = transform.normal_matrix().unwrap();

            assert!(matrix.is_finite());
            assert!((matrix * Vec3::splat(scale)).abs_diff_eq(Vec3::ONE, 1e-6));
            assert!(transform.matrix().unwrap().is_finite());
        }
    }

    #[test]
    fn test_transform_zero_scale() {
        let transform = Transform::new(
            Vec3::new(5.0, 7.0, 11.0),
            Quat::IDENTITY,
            Vec3::new(1.0, 0.0, 2.0),
        );
        let point = Vec3::new(3.0, 4.0, 5.0);

        assert!(transform.normal_matrix().is_err());
        assert_eq!(
            transform.matrix().unwrap().transform_point3(point),
            Vec3::new(8.0, 7.0, 21.0)
        );
        assert_eq!(
            transform.transform_point(&point).unwrap(),
            Vec3::new(8.0, 7.0, 21.0)
        );
        assert_eq!(
            transform.transform_vector(&point).unwrap(),
            Vec3::new(3.0, 0.0, 10.0)
        );
    }

    #[test]
    fn test_transform_normal_matrix_nonfinite_inverse() {
        let transform = Transform::new(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(f32::from_bits(1), 1.0, 1.0),
        );

        assert!(transform.matrix().is_ok());
        assert!(transform.normal_matrix().is_err());
    }

    #[test]
    fn test_transform_invalid_rotation() {
        for rotation in [
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            Quat::from_xyzw(0.0, 0.0, 0.0, 2.0),
            Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
            Quat::from_xyzw(0.0, f32::INFINITY, 0.0, 1.0),
        ] {
            let transform = Transform::new(Vec3::ZERO, rotation, Vec3::ONE);

            assert!(transform.matrix().is_err());
            assert!(transform.normal_matrix().is_err());
            assert!(transform.transform_point(&Vec3::ONE).is_err());
            assert!(transform.transform_vector(&Vec3::ONE).is_err());
        }
    }

    #[test]
    fn test_transform_invalid_scale() {
        for scale in [
            Vec3::new(f32::NAN, 1.0, 1.0),
            Vec3::new(1.0, f32::INFINITY, 1.0),
            Vec3::new(1.0, 1.0, f32::NEG_INFINITY),
        ] {
            let transform = Transform::new(Vec3::ZERO, Quat::IDENTITY, scale);

            assert!(transform.matrix().is_err());
            assert!(transform.normal_matrix().is_err());
            assert!(transform.transform_point(&Vec3::ONE).is_err());
            assert!(transform.transform_vector(&Vec3::ONE).is_err());
        }
    }

    #[test]
    fn test_transform_invalid_translation() {
        for translation in [
            Vec3::new(f32::NAN, 0.0, 0.0),
            Vec3::new(0.0, f32::INFINITY, 0.0),
            Vec3::new(0.0, 0.0, f32::NEG_INFINITY),
        ] {
            let transform = Transform::new(translation, Quat::IDENTITY, Vec3::ONE);

            assert!(transform.matrix().is_err());
            assert!(transform.transform_point(&Vec3::ONE).is_err());
            assert_eq!(transform.normal_matrix().unwrap(), Mat3::IDENTITY);
            assert_eq!(transform.transform_vector(&Vec3::ONE).unwrap(), Vec3::ONE);
        }
    }

    #[test]
    fn test_transform_nonfinite_input() {
        let transform = Transform::IDENTITY;
        for input in [
            Vec3::new(f32::NAN, 0.0, 0.0),
            Vec3::new(0.0, f32::INFINITY, 0.0),
            Vec3::new(0.0, 0.0, f32::NEG_INFINITY),
        ] {
            assert!(transform.transform_point(&input).is_err());
            assert!(transform.transform_vector(&input).is_err());
        }
    }

    #[test]
    fn test_transform_vector_overflow() {
        let transform = Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::splat(2.0));
        let vector = Vec3::new(f32::MAX, 0.0, 0.0);

        assert!(transform.matrix().is_ok());
        assert!(transform.transform_vector(&vector).is_err());
        assert!(transform.transform_point(&vector).is_err());
    }

    #[test]
    fn test_transform_point_overflow() {
        let transform = Transform::new(Vec3::new(f32::MAX, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE);
        let point = Vec3::new(f32::MAX, 0.0, 0.0);

        assert_eq!(transform.transform_vector(&point).unwrap(), point);
        assert!(transform.transform_point(&point).is_err());
    }
}
