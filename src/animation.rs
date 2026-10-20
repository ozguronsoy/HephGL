use std::collections::HashSet;

use crate::{
    Error, Result,
    math::{Quat, Vec4},
    transform::Transform,
};

/// The local transform property controlled by an animation channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationProperty {
    /// The translation along the X, Y, and Z axes.
    Translation,
    /// The rotation represented by an XYZW quaternion.
    Rotation,
    /// The scale along the X, Y, and Z axes.
    Scale,
}

/// Defines how values between animation keyframes are calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationInterpolation {
    /// No interpolation between keyframes.
    Step,
    /// Interpolates vectors linearly and rotations along the shortest spherical path.
    Linear,
    /// Uses cubic Hermite interpolation with incoming and outgoing tangents.
    CubicSpline,
}

/// Keyframes controlling one property of a joint.
#[derive(Clone)]
pub struct AnimationChannel {
    /// The target joint's index in the skeleton.
    pub joint_index: usize,
    /// The local transform property controlled by this channel.
    pub property: AnimationProperty,
    /// The interpolation applied between keyframes.
    pub interpolation: AnimationInterpolation,
    /// Increasing, nonnegative keyframe times in seconds.
    pub times: Vec<f32>,
    /// XYZ vectors with unused W, or XYZW rotations. Cubic channels store incoming tangent, value,
    /// and outgoing tangent for each keyframe.
    pub values: Vec<Vec4>,
}

/// A reusable clip that samples transforms.
#[derive(Clone)]
pub struct Animation {
    channels: Vec<AnimationChannel>,
    duration: f32,
}

impl Animation {
    /// Creates a clip from channels, rejecting invalid keyframes and duplicate target properties.
    pub fn new(channels: Vec<AnimationChannel>) -> Result<Self> {
        let mut duration: f32 = 0.0;
        let mut targets = HashSet::new();
        for channel in &channels {
            let stride = match channel.interpolation == AnimationInterpolation::CubicSpline {
                true => 3,
                false => 1,
            };

            if !targets.insert((channel.joint_index, channel.property)) {
                return Err(Error::invalid_argument(
                    "Animation has duplicate channels for a joint property.",
                ));
            }
            if channel.times.is_empty()
                || channel.times[0] < 0.0
                || channel.times.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(Error::invalid_argument(
                    "Animation keyframe times must be nonnegative and strictly increasing.",
                ));
            }
            if channel.values.len() != channel.times.len() * stride {
                return Err(Error::invalid_argument(
                    "Animation keyframe time and value counts differ.",
                ));
            }

            if channel.property == AnimationProperty::Rotation {
                let offset = match stride == 3 {
                    true => 1,
                    false => 0,
                };
                for value in channel.values.iter().skip(offset).step_by(stride) {
                    if value.length_squared() <= 0.0 {
                        return Err(Error::invalid_argument(
                            "Animation rotation has zero length.",
                        ));
                    }
                }
            }
            duration = duration.max(channel.times[channel.times.len() - 1]);
        }

        Ok(Self { channels, duration })
    }

    /// Gets the animation channels.
    pub fn channels(&self) -> &[AnimationChannel] {
        &self.channels
    }

    /// Gets the duration in seconds.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Samples into compatible local transforms, clamping channel times and leaving unanimated
    /// properties unchanged.
    pub fn sample(&self, time: f32, transforms: &mut [Transform]) -> Result<()> {
        if time < 0.0 {
            return Err(Error::invalid_argument(
                "Animation time must be a nonegative number.",
            ));
        }
        if self
            .channels
            .iter()
            .any(|channel| channel.joint_index >= transforms.len())
        {
            return Err(Error::invalid_argument(
                "Animation joint index is out of range.",
            ));
        }

        for channel in &self.channels {
            let value = channel.sample(time)?;
            let transform = &mut transforms[channel.joint_index];
            match channel.property {
                AnimationProperty::Translation => transform.translation = value.truncate(),
                AnimationProperty::Rotation => {
                    transform.rotation = Quat::from_array(value.to_array())
                }
                AnimationProperty::Scale => transform.scale = value.truncate(),
            }
        }

        Ok(())
    }
}

impl AnimationChannel {
    fn sample(&self, time: f32) -> Result<Vec4> {
        let upper = self.times.partition_point(|&key_time| key_time <= time);
        let lower = upper.saturating_sub(1);
        let upper = upper.min(self.times.len() - 1);
        let cubic = self.interpolation == AnimationInterpolation::CubicSpline;
        let stride = if cubic { 3 } else { 1 };
        let offset = usize::from(cubic);
        let first = self.values[lower * stride + offset];
        let second = self.values[upper * stride + offset];

        let mut value = if lower == upper {
            first
        } else {
            let duration = self.times[upper] - self.times[lower];
            let progress = (time - self.times[lower]) / duration;
            match self.interpolation {
                AnimationInterpolation::Step => first,
                AnimationInterpolation::Linear => {
                    if self.property == AnimationProperty::Rotation {
                        Vec4::from_array(
                            Quat::from_array(first.normalize().to_array())
                                .slerp(Quat::from_array(second.normalize().to_array()), progress)
                                .to_array(),
                        )
                    } else {
                        first.lerp(second, progress)
                    }
                }
                AnimationInterpolation::CubicSpline => {
                    let squared = progress * progress;
                    let cubed = squared * progress;
                    first * (2.0 * cubed - 3.0 * squared + 1.0)
                        + self.values[lower * 3 + 2] * duration * (cubed - 2.0 * squared + progress)
                        + second * (-2.0 * cubed + 3.0 * squared)
                        + self.values[upper * 3] * duration * (cubed - squared)
                }
            }
        };

        if self.property == AnimationProperty::Rotation {
            if value.length_squared() <= 0.0 {
                return Err(Error::invalid_argument(
                    "Sampled animation rotation has zero length.",
                ));
            }
            value = value.normalize();
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;

    #[test]
    fn test_sampling() {
        let channel = AnimationChannel {
            joint_index: 0,
            property: AnimationProperty::Translation,
            interpolation: AnimationInterpolation::Linear,
            times: vec![1.0, 3.0],
            values: vec![Vec4::ZERO, Vec4::new(4.0, 0.0, 0.0, 0.0)],
        };
        let animation = Animation::new(vec![channel.clone()]).unwrap();
        let mut transforms = [Transform::IDENTITY];
        animation.sample(2.0, &mut transforms).unwrap();
        assert_eq!(transforms[0].translation, Vec3::X * 2.0);
        animation.sample(0.0, &mut transforms).unwrap();
        assert_eq!(transforms[0].translation, Vec3::ZERO);
        animation.sample(4.0, &mut transforms).unwrap();
        assert_eq!(transforms[0].translation, Vec3::X * 4.0);
        assert!(animation.sample(0.0, &mut []).is_err());
        let step = Animation::new(vec![AnimationChannel {
            interpolation: AnimationInterpolation::Step,
            ..channel.clone()
        }])
        .unwrap();
        step.sample(2.0, &mut transforms).unwrap();
        assert_eq!(transforms[0].translation, Vec3::ZERO);
        step.sample(3.0, &mut transforms).unwrap();
        assert_eq!(transforms[0].translation, Vec3::X * 4.0);
        let cubic = Animation::new(vec![AnimationChannel {
            interpolation: AnimationInterpolation::CubicSpline,
            values: vec![
                Vec4::ZERO,
                Vec4::ZERO,
                Vec4::X,
                Vec4::ZERO,
                Vec4::ZERO,
                Vec4::ZERO,
            ],
            ..channel
        }])
        .unwrap();
        cubic.sample(2.0, &mut transforms).unwrap();
        assert_eq!(transforms[0].translation.x, 0.25);
    }

    #[test]
    fn test_rotations_and_validation() {
        let channel = AnimationChannel {
            joint_index: 0,
            property: AnimationProperty::Rotation,
            interpolation: AnimationInterpolation::Linear,
            times: vec![0.0, 2.0],
            values: vec![
                Vec4::W,
                -Vec4::from_array(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array()),
            ],
        };
        let animation = Animation::new(vec![channel.clone()]).unwrap();
        let mut transforms = [Transform::IDENTITY];
        animation.sample(1.0, &mut transforms).unwrap();
        assert!(
            transforms[0]
                .rotation
                .abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4), 1e-6)
        );
        let cubic = Animation::new(vec![AnimationChannel {
            interpolation: AnimationInterpolation::CubicSpline,
            values: vec![
                Vec4::ZERO,
                Vec4::W,
                Vec4::ZERO,
                Vec4::ZERO,
                -channel.values[1],
                Vec4::ZERO,
            ],
            ..channel.clone()
        }])
        .unwrap();
        cubic.sample(1.0, &mut transforms).unwrap();
        assert!(transforms[0].rotation.is_normalized());
        assert!(
            transforms[0]
                .rotation
                .abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4), 1e-6)
        );
        assert!(Animation::new(vec![channel.clone(), channel.clone()]).is_err());
        assert!(
            Animation::new(vec![AnimationChannel {
                times: vec![0.0, 0.0],
                ..channel.clone()
            }])
            .is_err()
        );
        assert!(
            Animation::new(vec![AnimationChannel {
                values: vec![Vec4::ZERO; 2],
                ..channel.clone()
            }])
            .is_err()
        );
        assert!(
            Animation::new(vec![AnimationChannel {
                times: vec![],
                ..channel
            }])
            .is_err()
        );
    }
}
