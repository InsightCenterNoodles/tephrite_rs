use crate::replication::instruction::*;
use crate::replication::sets::*;
use crate::serialize::transcript_writer::TranscriptWriteStateResource;
use crate::serialize::*;
use bevy::prelude::*;

use crate::serialize::create_serialize_enum;

// TODO: The standard mat makes the enum explode in size

macro_rules! make_change_detection {
    ($app:ident, $A:tt) => {
        $app.add_systems(
            Last,
            (|mut ev_asset: MessageReader<AssetEvent<$A>>,
              assets: Res<Assets<$A>>,
              mut writer: NonSendMut<TranscriptWriteStateResource>| {
                //println!("Checking for deltas to {}", stringify!($A));
                for e in ev_asset.read() {
                    debug!("EVENT {e:?}");
                    match e {
                        AssetEvent::Added { id } => {
                            let asset = assets.get(*id).expect("obtaining new asset");

                            //debug!("NEW ASSET {asset:?}");

                            let dest: &mut TranscriptWriteStateResource = &mut writer;

                            unsafe {
                                ServerInstruction::CAsset(ServerReplicateAsset {
                                    asset: AssetEnumRef::convert(*id, asset),
                                })
                                .write_fast(dest);
                            }
                        }
                        AssetEvent::Modified { id } => {
                            let asset = assets.get(*id).expect("obtaining changed asset");

                            let dest: &mut TranscriptWriteStateResource = &mut writer;

                            unsafe {
                                ServerInstruction::CAsset(ServerReplicateAsset {
                                    asset: AssetEnumRef::convert(*id, asset),
                                })
                                .write_fast(dest);
                            }
                        }
                        AssetEvent::Removed { id } => {
                            let dest: &mut TranscriptWriteStateResource = &mut writer;
                            unsafe {
                                ServerInstruction::CDropAsset(DropAsset { id: (*id).into() })
                                    .write_fast(dest);
                            }
                        }
                        _ => {} // do nothing for NOW
                                //AssetEvent::Unused { id } => todo!(),
                                //AssetEvent::LoadedWithDependencies { id } => todo!(),
                    }
                }
            })
            .in_set(AssetDeltaPhase),
        );
    };
}

#[derive(Debug)]
pub(crate) struct ReplicatedAsset<T: Asset> {
    pub(crate) id: AssetId<T>,
    pub(crate) data: T,
}

impl<T: Asset + FastWrite> FastWrite for ReplicatedAsset<T> {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.id.write_fast(w);
            self.data.write_fast(w);
        };
    }
}

impl<T: Asset + FastRead<Ret = T>> FastRead for ReplicatedAsset<T> {
    type Ret = Self;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        Self {
            id: read_fast(r),
            data: read_fast(r),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ReplicatedAssetRef<'a, T: Asset> {
    pub(crate) id: AssetId<T>,
    pub(crate) data: &'a T,
}

impl<'b, T: Asset + FastWrite> FastWrite for ReplicatedAssetRef<'b, T> {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.id.write_fast(w);
            self.data.write_fast(w);
        };
    }
}

pub(crate) trait ConvertAsset<'a, T: Asset> {
    fn convert(id: AssetId<T>, t: &'a T) -> Self;
}

macro_rules! generate_asset_systems {
    (
        $( ( $id:expr, $T:tt ) ),* $(,)?
    ) => {

        create_serialize_enum!(
            ReplicatedAssetID,
            u8,
            {
                $( ($id, $T, AssetId<$T>), )*
            }
        );

         $(
            impl From<AssetId<$T>> for ReplicatedAssetID {
                fn from(v: AssetId<$T>) -> Self {
                    Self::$T(v)
                }
            }
        )*

        // Replicated asset

        create_serialize_enum!(
            AssetEnum,
            u8,
            {
                $( ($id, $T, ReplicatedAsset<$T>), )*
            }
        );

        create_serialize_enum_write_only!(
            AssetEnumRef,
            u8,
            lifetime: 'a,
            {
                $( ($id, $T, ReplicatedAssetRef<'a, $T>), )*
            }
        );

        $(
            impl<'a> ConvertAsset<'a, $T> for AssetEnumRef<'a> {
                fn convert(id: AssetId<$T>, t: &'a $T) -> Self {
                    AssetEnumRef::$T(ReplicatedAssetRef {
                        id,
                        data: t,
                    })
                }
            }
        )*

        // impl ReplicatedAssetID {
        //     pub fn remove_asset(self, e: Entity, commands: &mut Commands) {
        //         match self {
        //             $(
        //                 ReplicatedComponentID::$T => commands.entity(e).remove::<$T>(),
        //             )*
        //         };
        //     }
        // }

        // impl AssetEnum {
        //     pub fn add_component(self, e: Entity, commands: &mut Commands) {
        //         match self {
        //             $(
        //                 ReplicatedComponent::$T(x) => commands.entity(e).insert(x),
        //             )*
        //         };
        //     }
        // }

        pub(crate) fn setup_replicated_asset_systems(app: &mut App) {
            $(
                make_change_detection!(app, $T);
            )*
        }

    };
}

generate_asset_systems!((0, Mesh), (1, StandardMaterial), (2, Image));
