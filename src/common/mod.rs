use crate::transcript;

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
