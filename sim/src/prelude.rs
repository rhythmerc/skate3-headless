//! What the sim modules used to get from `bevy::prelude::*`: glam's math types (the exact glam
//! Bevy 0.18 uses, so they are the same types on both sides), the `tracing` macros Bevy's log
//! macros are, and a plain [`Transform`].
pub use glam::{
    Affine3A, BVec3, EulerRot, IVec2, IVec3, Mat2, Mat3, Mat3A, Mat4, Quat, UVec2, UVec3, Vec2,
    Vec3, Vec3A, Vec4,
};
pub use tracing::{debug, error, info, info_span, trace, warn};

/// Bevy's `Transform` (translation, rotation, scale) without Bevy: the same fields and the same
/// arithmetic for the handful of operations the sim uses, so results are bit-identical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    #[inline]
    pub fn from_matrix(world_from_local: Mat4) -> Self {
        let (scale, rotation, translation) = world_from_local.to_scale_rotation_translation();
        Transform {
            translation,
            rotation,
            scale,
        }
    }

    #[inline]
    pub const fn from_translation(translation: Vec3) -> Self {
        Transform {
            translation,
            ..Self::IDENTITY
        }
    }

    #[inline]
    pub const fn from_rotation(rotation: Quat) -> Self {
        Transform {
            rotation,
            ..Self::IDENTITY
        }
    }

    #[inline]
    #[must_use]
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    #[inline]
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// `bevy::prelude::default`.
#[inline]
pub fn default<T: Default>() -> T {
    T::default()
}
