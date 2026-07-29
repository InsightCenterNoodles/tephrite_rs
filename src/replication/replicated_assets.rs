use bevy::ecs::system::SystemState;
use bevy::prelude::*;

use crate::prelude::PointsMaterial;
use crate::replication::instruction::*;
use crate::serialize::create_serialize_enum;
use crate::serialize::transcript_writer::TranscriptWriteStateResource;
use crate::serialize::*;

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
    ($( ( $id:expr, $T:tt, $P:expr ) ),* $(,)?) => {
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
                    AssetEnumRef::$T(ReplicatedAssetRef { id, data: t })
                }
            }
        )*

        pub(crate) fn init_replicated_asset_readers(world: &mut World) {
            $( init_asset_reader::<$T>(world); )*
        }

        pub(crate) fn write_changed_assets(
            world: &mut World,
            dest: &mut TranscriptWriteStateResource,
        ) {
            $( write_asset_changes::<$T>(world, dest); )*
        }
    };
}

#[derive(Resource)]
struct CachedAssetReader<A: Asset> {
    state: SystemState<(
        MessageReader<'static, 'static, AssetEvent<A>>,
        Res<'static, Assets<A>>,
    )>,
}

fn init_asset_reader<A: Asset>(world: &mut World) {
    if !world.contains_resource::<CachedAssetReader<A>>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedAssetReader::<A> { state });
    }
}

fn write_asset_changes<A>(world: &mut World, dest: &mut TranscriptWriteStateResource)
where
    A: Asset + FastWrite,
    for<'a> AssetEnumRef<'a>: ConvertAsset<'a, A>,
    ReplicatedAssetID: From<AssetId<A>>,
{
    init_asset_reader::<A>(world);

    world.resource_scope(|world, mut cached: Mut<CachedAssetReader<A>>| {
        let Ok((mut events, assets)) = cached.state.get_mut(world) else {
            return;
        };

        for event in events.read() {
            match event {
                AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                    if let Some(asset) = assets.get(*id) {
                        unsafe {
                            ServerInstruction::CAsset(ServerReplicateAsset {
                                asset: AssetEnumRef::convert(*id, asset),
                            })
                            .write_fast(dest);
                        }
                    }
                }
                AssetEvent::Removed { id } => unsafe {
                    ServerInstruction::CDropAsset(DropAsset { id: (*id).into() }).write_fast(dest);
                },
                AssetEvent::Unused { id: _ } | AssetEvent::LoadedWithDependencies { id: _ } => {}
            }
        }

        cached.state.apply(world);
    });
}

generate_asset_systems!(
    (0, Mesh, ()),
    (1, StandardMaterial, ()),
    (2, PointsMaterial, ()),
    (3, Font, ()),
    (4, Image, ()),
);
