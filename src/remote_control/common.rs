use bevy::prelude::Vec3;

pub(crate) const INDEX_PATH: &str = "/";
pub(crate) const EVENT_PATH: &str = "/event";
pub(crate) const QUIT_ID: &str = "__tephrite_quit";

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Numeric value, used by slider controls.
    Float(f32),
    /// Boolean value, used by toggle controls.
    Bool(bool),
    /// String choice, used by select controls.
    Choice(String),
    /// Free-form text value.
    Text(String),
    /// 3D vector value.
    Vec3(Vec3),
    /// Stateless trigger, used by button controls.
    Triggered,
}
