use crate::transcript;

/// Marker for the entity that represents the user's head
#[derive(bevy::prelude::Component, Debug)]
pub(crate) struct Head;

impl transcript::TSerialize for Head {
    fn serialize(&self, _: &mut impl std::io::Write) {}
}

impl transcript::TDeserialize for Head {
    fn deserialize(_: &mut impl std::io::Read) -> Self {
        Self
    }
}
