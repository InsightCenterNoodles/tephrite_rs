use bevy::prelude::Vec3;

/// URL path for the remote-control HTML page.
pub(crate) const INDEX_PATH: &str = "/";
/// URL path for property update POST requests.
pub(crate) const EVENT_PATH: &str = "/event";
/// Special control ID used to request app shutdown.
pub(crate) const QUIT_ID: &str = "__tephrite_quit";

/// Typed payload sent with [`crate::remote_control::events::RemoteControlEvent`].
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

impl TryFrom<PropertyValue> for f32 {
    type Error = &'static str;

    fn try_from(value: PropertyValue) -> Result<Self, Self::Error> {
        match value {
            PropertyValue::Float(x) => Ok(x),
            PropertyValue::Bool(x) => Ok((x as u32) as Self),
            PropertyValue::Choice(_) => Err("unable to convert choice to f32"),
            PropertyValue::Text(x) => x.parse().map_err(|_| "unable to convert text to f32"),
            PropertyValue::Vec3(vec3) => Ok(vec3.x),
            PropertyValue::Triggered => Ok(1.0),
        }
    }
}

impl TryFrom<PropertyValue> for bool {
    type Error = &'static str;

    fn try_from(value: PropertyValue) -> Result<Self, Self::Error> {
        match value {
            PropertyValue::Float(x) => Ok(x != 0.0),
            PropertyValue::Bool(x) => Ok(x),
            PropertyValue::Choice(x) => Ok(!x.is_empty()),
            PropertyValue::Text(x) => {
                if x.eq_ignore_ascii_case("true") || x == "1" {
                    Ok(true)
                } else if x.eq_ignore_ascii_case("false") || x == "0" {
                    Ok(false)
                } else {
                    Err("unable to convert text to bool")
                }
            }
            PropertyValue::Vec3(vec3) => Ok(vec3.x != 0.0 || vec3.y != 0.0 || vec3.z != 0.0),
            PropertyValue::Triggered => Ok(true),
        }
    }
}

impl TryFrom<PropertyValue> for String {
    type Error = &'static str;

    fn try_from(value: PropertyValue) -> Result<Self, Self::Error> {
        match value {
            PropertyValue::Float(x) => Ok(x.to_string()),
            PropertyValue::Bool(x) => Ok(x.to_string()),
            PropertyValue::Choice(x) => Ok(x),
            PropertyValue::Text(x) => Ok(x),
            PropertyValue::Vec3(vec3) => Ok(format!("({}, {}, {})", vec3.x, vec3.y, vec3.z)),
            PropertyValue::Triggered => Ok("triggered".to_string()),
        }
    }
}

impl TryFrom<PropertyValue> for Vec3 {
    type Error = &'static str;

    fn try_from(value: PropertyValue) -> Result<Self, Self::Error> {
        match value {
            PropertyValue::Float(x) => Ok(Vec3::new(x, 0.0, 0.0)),
            PropertyValue::Bool(x) => Ok(Vec3::new((x as u32) as f32, 0.0, 0.0)),
            PropertyValue::Choice(x) => {
                let parts: Vec<&str> = x.split(',').collect();
                if parts.len() >= 3 {
                    let x = parts[0]
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| "unable to parse x component")?;
                    let y = parts[1]
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| "unable to parse y component")?;
                    let z = parts[2]
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| "unable to parse z component")?;
                    Ok(Vec3::new(x, y, z))
                } else {
                    Err("unable to convert choice to Vec3")
                }
            }
            PropertyValue::Text(x) => {
                let parts: Vec<&str> = x.split(',').collect();
                if parts.len() >= 3 {
                    let x = parts[0]
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| "unable to parse x component")?;
                    let y = parts[1]
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| "unable to parse y component")?;
                    let z = parts[2]
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| "unable to parse z component")?;
                    Ok(Vec3::new(x, y, z))
                } else {
                    Err("unable to convert text to Vec3")
                }
            }
            PropertyValue::Vec3(vec3) => Ok(vec3),
            PropertyValue::Triggered => Ok(Vec3::new(0.0, 0.0, 0.0)),
        }
    }
}
