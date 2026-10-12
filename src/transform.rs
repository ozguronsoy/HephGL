use crate::math::{Mat3, Mat4, MathError, Quat, Vec3};

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
    pub fn matrix(&self) -> Result<Mat4, MathError> {
        self.validate_rotation_and_scale()?;
        if !self.translation.is_finite() {
            return Err(MathError::invalid_argument(
                "Transform translation must be finite.",
            ));
        }
        let matrix =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        if !matrix.is_finite() {
            return Err(MathError::invalid_argument(
                "Transform matrix must be finite.",
            ));
        }
        Ok(matrix)
    }

    /// Calculates the inverse transpose of the model matrix's linear part for transforming normals.
    pub fn normal_matrix(&self) -> Result<Mat3, MathError> {
        self.validate_rotation_and_scale()?;
        let matrix = Mat3::from_quat(self.rotation) * Mat3::from_diagonal(self.scale.recip());
        if !matrix.is_finite() {
            return Err(MathError::invalid_argument(
                "Transform linear matrix must have a finite inverse.",
            ));
        }
        Ok(matrix)
    }

    /// Transforms a local point into the parent coordinate system, including translation.
    pub fn transform_point(&self, point: &Vec3) -> Result<Vec3, MathError> {
        if !self.translation.is_finite() {
            return Err(MathError::invalid_argument(
                "Transform translation must be finite.",
            ));
        }
        let point = self.transform_vector(point)? + self.translation;
        if !point.is_finite() {
            return Err(MathError::invalid_argument(
                "Transformed point must be finite.",
            ));
        }
        Ok(point)
    }

    /// Transforms a local vector into the parent coordinate system without translation.
    pub fn transform_vector(&self, vector: &Vec3) -> Result<Vec3, MathError> {
        self.validate_rotation_and_scale()?;
        if !vector.is_finite() {
            return Err(MathError::invalid_argument(
                "Transform input must be finite.",
            ));
        }
        let vector = self.rotation * (self.scale * vector);
        if !vector.is_finite() {
            return Err(MathError::invalid_argument(
                "Transformed vector must be finite.",
            ));
        }
        Ok(vector)
    }

    fn validate_rotation_and_scale(&self) -> Result<(), MathError> {
        if !self.rotation.is_normalized() {
            return Err(MathError::invalid_argument(
                "Transform rotation must be normalized.",
            ));
        }
        if !self.scale.is_finite() {
            return Err(MathError::invalid_argument(
                "Transform scale must be finite.",
            ));
        }
        Ok(())
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
