use bevy::math::{Affine2, Mat2, Mat3, Vec2};
use bevy::math::{Affine3A, Mat3A, Quat, Vec3, Vec3A};

use super::common::byte_deserialize;
use super::common::byte_serialize;
use super::macros::raw_item_helper;
use super::TDeserialize;
use super::TSerialize;

// =============================================================================

raw_item_helper!(Vec2);

// =============================================================================

raw_item_helper!(Vec3);
raw_item_helper!(Vec3A);

// =============================================================================

raw_item_helper!(Quat);

// =============================================================================

raw_item_helper!(Mat2);
raw_item_helper!(Mat3);
raw_item_helper!(Mat3A);

// =============================================================================

raw_item_helper!(Affine2);
raw_item_helper!(Affine3A);
