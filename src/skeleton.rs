use crate::{Error, Result, math::Mat4, transform::Transform};

/// A joint in a skeleton.
#[derive(Debug, Clone, Copy)]
pub struct Joint {
    /// The parent joint's index, or `None` for a root.
    pub parent_index: Option<usize>,
    /// The joint's default transform relative to its parent.
    pub transform: Transform,
}

/// A joint hierarchy.
#[derive(Clone)]
pub struct Skeleton {
    joints: Vec<Joint>,
    joint_order: Vec<usize>,
}

impl Skeleton {
    /// Creates a skeleton, rejecting invalid parent indices and cycles.
    pub fn new(joints: Vec<Joint>) -> Result<Self> {
        let mut children = vec![Vec::new(); joints.len()];
        let mut joint_order = Vec::with_capacity(joints.len());
        for (index, joint) in joints.iter().enumerate() {
            match joint.parent_index {
                Some(parent) => children
                    .get_mut(parent)
                    .ok_or(Error::invalid_argument(
                        "Skeleton parent index is out of range.",
                    ))?
                    .push(index),
                None => joint_order.push(index),
            }
        }

        let mut index = 0;
        while index < joint_order.len() {
            joint_order.extend_from_slice(&children[joint_order[index]]);
            index += 1;
        }
        if joint_order.len() != joints.len() {
            return Err(Error::invalid_argument("Skeleton contains a cycle."));
        }

        Ok(Self {
            joints,
            joint_order,
        })
    }

    /// Gets the joints.
    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }

    /// Gets the default local transforms.
    pub fn transforms(&self) -> Vec<Transform> {
        self.joints.iter().map(|joint| joint.transform).collect()
    }

    /// Calculates global joint matrices from one local transform per joint.
    pub fn matrices(&self, transforms: &[Transform]) -> Result<Vec<Mat4>> {
        if transforms.len() != self.joints.len() {
            return Err(Error::invalid_argument(
                "Skeleton transform count must match its joint count.",
            ));
        }

        let mut matrices = vec![Mat4::IDENTITY; self.joints.len()];
        for &index in &self.joint_order {
            let local = transforms[index].matrix()?;
            matrices[index] = match self.joints[index].parent_index {
                Some(parent) => matrices[parent] * local,
                None => local,
            };
        }

        Ok(matrices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Quat, Vec3};

    #[test]
    fn test_hierarchy() {
        let skeleton = Skeleton::new(vec![
            Joint {
                parent_index: Some(1),
                transform: Transform::new(Vec3::X, Quat::IDENTITY, Vec3::ONE),
            },
            Joint {
                parent_index: None,
                transform: Transform::new(
                    Vec3::Y,
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    Vec3::ONE,
                ),
            },
        ])
        .unwrap();
        let mut transforms = skeleton.transforms();
        let matrices = skeleton.matrices(&transforms).unwrap();
        assert!(
            matrices[0]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::Y * 2.0, 1e-6)
        );
        transforms[1].translation = Vec3::ZERO;
        assert!(
            skeleton.matrices(&transforms).unwrap()[0]
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(Vec3::Y, 1e-6)
        );
        assert_eq!(skeleton.joints()[1].transform.translation, Vec3::Y);
        assert!(skeleton.matrices(&[]).is_err());
    }

    #[test]
    fn test_invalid_hierarchy() {
        assert!(
            Skeleton::new(vec![Joint {
                parent_index: Some(1),
                transform: Transform::IDENTITY
            }])
            .is_err()
        );
        assert!(
            Skeleton::new(vec![Joint {
                parent_index: Some(0),
                transform: Transform::IDENTITY
            }])
            .is_err()
        );
        assert!(
            Skeleton::new(vec![
                Joint {
                    parent_index: Some(1),
                    transform: Transform::IDENTITY
                },
                Joint {
                    parent_index: Some(0),
                    transform: Transform::IDENTITY
                },
            ])
            .is_err()
        );
        assert!(
            Skeleton::new(Vec::new())
                .unwrap()
                .matrices(&[])
                .unwrap()
                .is_empty()
        );
    }
}
