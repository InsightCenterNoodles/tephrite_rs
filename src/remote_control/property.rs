use bevy::prelude::*;
use itertools::Itertools;

use crate::remote_control::common::PropertyValue;

/// Supported UI controls for a remote property.
#[derive(Debug, Clone)]
pub enum PropertyControl {
    /// A numeric slider.
    Slider {
        /// Minimum slider value.
        min: f32,
        /// Maximum slider value.
        max: f32,
        /// Slider step size.
        step: f32,
        /// Initial slider value shown in the UI.
        initial: f32,
    },
    /// A checkbox toggle.
    Toggle {
        /// Initial checkbox state.
        initial: bool,
    },
    /// A dropdown select with fixed options.
    Select {
        /// Allowed option values.
        options: Vec<String>,
        /// Initial selected option index.
        initial: usize,
    },
    /// A free-form text field.
    String {
        /// Initial text value.
        initial: String,
    },
    /// A 3D vector editor (x, y, z).
    Vector3 {
        /// Initial vector value.
        initial: Vec3,
        /// Per-axis increment.
        step: f32,
    },
    /// A push button that emits [`PropertyValue::Triggered`].
    Button,
}

/// Declarative property description used to build the webpage.
#[derive(Debug, Clone)]
pub struct PropertyDefinition {
    /// Caller-provided property identifier.
    pub id: Entity,
    /// Secondary per-entity discriminator, allowing multiple controls per entity.
    pub aspect_id: u32,
    /// Human-readable label displayed on the page.
    pub label: String,
    /// Control type and configuration.
    pub control: PropertyControl,
}

impl PropertyDefinition {
    /// Stable URL-safe identifier used by the remote control form post payload.
    pub(crate) fn lookup_id(&self) -> String {
        format!("{}:{}", self.id.to_bits(), self.aspect_id)
    }
}

/// Convert a raw form value to the typed property payload expected by a control.
pub(crate) fn parse_property_value(
    control: &PropertyControl,
    provided_value: Option<&String>,
) -> Result<PropertyValue, &'static str> {
    match control {
        PropertyControl::Slider { .. } => {
            let Some(raw) = provided_value else {
                return Err("missing value");
            };
            let parsed = raw.parse::<f32>().map_err(|_| "invalid slider value")?;
            Ok(PropertyValue::Float(parsed))
        }
        PropertyControl::Toggle { .. } => match provided_value.map(String::as_str) {
            Some("1") | Some("true") | Some("on") => Ok(PropertyValue::Bool(true)),
            Some("0") | Some("false") | Some("off") => Ok(PropertyValue::Bool(false)),
            _ => Err("invalid toggle value"),
        },
        PropertyControl::Select { options, .. } => {
            let Some(raw) = provided_value else {
                return Err("missing value");
            };
            if options.iter().any(|opt| opt == raw) {
                Ok(PropertyValue::Choice(raw.clone()))
            } else {
                Err("invalid select value")
            }
        }
        PropertyControl::String { .. } => {
            let Some(raw) = provided_value else {
                return Err("missing value");
            };
            Ok(PropertyValue::Text(raw.clone()))
        }
        PropertyControl::Vector3 { .. } => {
            let Some(raw) = provided_value else {
                return Err("missing value");
            };
            let parts: [f32; 3] = raw
                .split(',')
                .map(str::trim)
                .take(3)
                .filter_map(|x| x.parse::<f32>().ok())
                .next_array()
                .ok_or("invalid vec3 value")?;

            Ok(PropertyValue::Vec3(parts.into()))
        }
        PropertyControl::Button => Ok(PropertyValue::Triggered),
    }
}
