//! Native animation-space conversions shared by the sim and its hosts.
use glam::{Mat4, Vec3};
use skate_core::animation::output::NativeMatrix;

/// A native model-space matrix (rows of four, translation in row 3) as a glam column matrix.
pub fn native_matrix(matrix: NativeMatrix) -> Mat4 {
    Mat4::from_cols(
        Vec3::from_array(matrix[0][..3].try_into().unwrap()).extend(0.0),
        Vec3::from_array(matrix[1][..3].try_into().unwrap()).extend(0.0),
        Vec3::from_array(matrix[2][..3].try_into().unwrap()).extend(0.0),
        Vec3::from_array(matrix[3][..3].try_into().unwrap()).extend(1.0),
    )
}
