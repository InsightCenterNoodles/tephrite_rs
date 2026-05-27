use std::{str::FromStr, string::String};

use bevy::log::error;
use serde::Deserialize;

#[derive(Debug, Default, Clone)]
pub struct VRPNAddress {
    pub sender: String,
    pub host: String,
    pub port: u16,
    pub sensor: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum VRPNAddressParseError {
    #[error("Missing address part {0}")]
    MissingPart(String),
    #[error("Bad port {0}")]
    BadPort(#[from] std::num::ParseIntError),
    #[error("Invalid sensor {0}")]
    BadSensor(String),
}

impl FromStr for VRPNAddress {
    type Err = VRPNAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Should be in the form of sender/sensor@host:port
        let (sender, endpoint) = s
            .split_once('@')
            .ok_or_else(|| VRPNAddressParseError::MissingPart("host".into()))?;
        let (host, port) = endpoint
            .split_once(':')
            .ok_or_else(|| VRPNAddressParseError::MissingPart("port".into()))?;

        if sender.is_empty() {
            return Err(VRPNAddressParseError::MissingPart("sender".into()));
        }

        if host.is_empty() {
            return Err(VRPNAddressParseError::MissingPart("host".into()));
        }

        if port.is_empty() || port.contains(':') || port.contains('@') {
            return Err(VRPNAddressParseError::MissingPart("port".into()));
        }

        if sender.contains('@') || host.contains('@') || host.contains(':') {
            return Err(VRPNAddressParseError::MissingPart("address".into()));
        }

        let port: u16 = port.parse()?;

        let mut sensor: Option<u16> = None;

        let sender = if let Some((sndr, snsr)) = sender.split_once('/') {
            if sndr.is_empty() {
                return Err(VRPNAddressParseError::MissingPart("sender".into()));
            }

            if snsr.is_empty() || snsr.contains('/') {
                return Err(VRPNAddressParseError::BadSensor(snsr.into()));
            }

            sensor = Some(snsr.parse().map_err(|err: std::num::ParseIntError| {
                VRPNAddressParseError::BadSensor(err.to_string())
            })?);
            sndr
        } else {
            sender
        };

        Ok(Self {
            sender: sender.into(),
            host: host.into(),
            port,
            sensor,
        })
    }
}

impl<'de> Deserialize<'de> for VRPNAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

pub(crate) fn deserialize_legacy_vrpn_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<VRPNAddress>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(raw) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    Ok(Some(
        raw.split(',')
            .filter_map(|address| {
                VRPNAddress::from_str(address.trim())
                    .inspect_err(|err| error!("Error parsing VRPN address: {err}"))
                    .ok()
            })
            .collect(),
    ))
}

/// Named coordinate transforms for VRPN tracker poses.
#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VRPNCoordinateTransform {
    /// Preserve the historical Tephrite VRPN mapping: position `[-x, z, y]`
    /// and rotation `[-x, z, y, w]`.
    #[serde(alias = "vrpn_bevy")]
    #[default]
    Vicon,
    /// Use VRPN position and quaternion components as-is.
    Identity,
}
