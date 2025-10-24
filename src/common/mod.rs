use crate::serialize;

/// Marker for the entity that represents the user's head
#[derive(bevy::prelude::Component, Debug)]
pub struct Head;

impl serialize::FastWrite for Head {
    unsafe fn write_fast(&self, _w: &mut impl serialize::ByteSink) {
        // nothing to do.
    }
}

impl serialize::FastRead for Head {
    type Ret = Self;

    unsafe fn read_fast<'a, S: serialize::ByteSource<'a>>(_r: &mut S) -> Self::Ret {
        Self
    }
}
