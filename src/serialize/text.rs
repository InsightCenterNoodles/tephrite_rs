use crate::serialize::*;
use bevy::{platform::collections::HashMap, prelude::*, text::FontSmoothing};
use std::sync::{LazyLock, RwLock};

impl_fast_newtype!(TextColor);

static P_MAP: LazyLock<RwLock<HashMap<AssetId<Font>, Handle<Font>>>> =
    LazyLock::new(|| Default::default());

impl RemappableAsset for Font {
    #[inline]
    fn with_remapper<F: FnOnce(&HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&P_MAP.read().unwrap());
    }
    #[inline]
    fn with_remapper_mut<F: FnOnce(&mut HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&mut P_MAP.write().unwrap());
    }

    fn remap_to_local_or_reserve(id: AssetId<Self>, assets: &mut Assets<Self>) -> Handle<Self>
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        if id == AssetId::default() {
            // dbg!("DEFAULT ASSET FONT");
            return Handle::<Font>::default();
        }

        if let Some(handle) = Self::remap_to_local(id) {
            return handle;
        }

        let local = assets.reserve_handle();

        // warn!(
        //     "Missing asset mapping for {} id {id}; reserving client-local placeholder {}",
        //     std::any::type_name::<Self>(),
        //     local.id()
        // );

        Self::with_remapper_mut(|map| {
            map.insert(id, local.clone());
        });

        local
    }
}

impl FastWrite for FontSource {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        match self {
            FontSource::Handle(handle) => unsafe {
                0u8.write_fast(w);
                handle.write_fast(w);
            },
            FontSource::Family(smol_str) => unsafe {
                1u8.write_fast(w);
                smol_str.as_str().write_fast(w);
            },
            FontSource::Serif => unsafe {
                2u8.write_fast(w);
            },
            FontSource::SansSerif => unsafe {
                3u8.write_fast(w);
            },
            FontSource::Cursive => unsafe {
                4u8.write_fast(w);
            },
            FontSource::Fantasy => unsafe {
                5u8.write_fast(w);
            },
            FontSource::Monospace => unsafe {
                6u8.write_fast(w);
            },
            FontSource::SystemUi => unsafe {
                7u8.write_fast(w);
            },
            FontSource::UiSerif => unsafe {
                8u8.write_fast(w);
            },
            FontSource::UiSansSerif => unsafe {
                9u8.write_fast(w);
            },
            FontSource::UiMonospace => unsafe {
                10u8.write_fast(w);
            },
            FontSource::UiRounded => unsafe {
                11u8.write_fast(w);
            },
            FontSource::Emoji => unsafe {
                12u8.write_fast(w);
            },
            FontSource::Math => unsafe {
                13u8.write_fast(w);
            },
            FontSource::FangSong => unsafe {
                14u8.write_fast(w);
            },
        }
    }
}

impl FastRead for FontSource {
    type Ret = Self;
    type Context = Assets<Font>;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(c: &mut Self::Context, r: &mut S) -> Self::Ret {
        match unsafe { u8::read_fast(&mut (), r) } {
            0 => unsafe { FontSource::Handle(Handle::read_fast(c, r)) },
            1 => unsafe { FontSource::Family(String::read_fast(&mut (), r).into()) },
            2 => FontSource::Serif,
            3 => FontSource::SansSerif,
            4 => FontSource::Cursive,
            5 => FontSource::Fantasy,
            6 => FontSource::Monospace,
            7 => FontSource::SystemUi,
            8 => FontSource::UiSerif,
            9 => FontSource::UiSansSerif,
            10 => FontSource::UiMonospace,
            11 => FontSource::UiRounded,
            12 => FontSource::Emoji,
            13 => FontSource::Math,
            14 => FontSource::FangSong,
            _ => panic!("unknown font source type. this should not happen."),
        }
    }
}

impl_fast_newtype!(FontWeight);
impl_fast_newtype!(FontWidth);
impl_fast_raw_item!(FontSmoothing);
impl_fast_raw_item!(FontStyle);
impl_fast_raw_item!(FontSize);

impl crate::serialize::fast_ser::FastWrite for TextFont {
    #[inline(always)]
    #[allow(unused)]
    unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
        unsafe { self.font.write_fast(w) };
        unsafe { self.font_size.write_fast(w) };
        unsafe { self.weight.write_fast(w) };
        unsafe { self.font_smoothing.write_fast(w) };
        unsafe { self.width.write_fast(w) };
        unsafe { self.style.write_fast(w) };
    }
}
impl crate::serialize::fast_ser::FastRead for TextFont {
    type Ret = TextFont;
    type Context = Assets<Font>;

    unsafe fn read_fast<'z, S: crate::serialize::fast_io::ByteSource<'z>>(
        c: &mut Self::Context,
        r: &mut S,
    ) -> Self {
        #[allow(unused)]
        use crate::serialize::fast_ser::read_fast;
        let nc = &mut ();
        Self {
            font: read_fast(c, r),
            font_size: read_fast(nc, r),
            weight: read_fast(nc, r),
            font_smoothing: read_fast(nc, r),
            width: read_fast(nc, r),
            style: read_fast(nc, r),
            font_features: Default::default(),
            font_variations: Default::default(),
        }
    }
}

impl_fast_raw_item!(Justify);
impl_fast_raw_item!(LineBreak);

impl_fast_serialize!(
    TextLayout,
    (),
    keep: {
        justify,
        linebreak
    }, skip: {
    }
);

impl_fast_newtype!(TextSpan);

impl FastWrite for Font {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.data.data().write_fast(w);
            self.alias.write_fast(w);
        };
    }
}

impl FastRead for Font {
    type Ret = Self;
    type Context = ();

    unsafe fn read_fast<'a, S: ByteSource<'a>>(c: &mut Self::Context, r: &mut S) -> Self::Ret {
        let mut ret = Font::from_bytes(unsafe { Vec::<u8>::read_fast(c, r) });

        ret.alias = unsafe { String::read_fast(c, r) };

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_font_id_remaps_to_default_font_handle() {
        let mut assets = Assets::<Font>::default();
        let handle = Font::remap_to_local_or_reserve(AssetId::default(), &mut assets);

        assert_eq!(handle.id(), Handle::<Font>::default().id());
    }
}
