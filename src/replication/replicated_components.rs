use bevy::ecs::{entity::EntityHashSet, system::SystemState};
use bevy::light::cascade::CascadeShadowConfig;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use super::instruction::*;
use crate::common::Head;
use crate::prelude::PointsMaterial;
use crate::replication::components::IsReplicated;
use crate::serialize::transcript_writer::TranscriptWriteStateResource;
use crate::serialize::*;

macro_rules! component_helpers {
    (collect, $world:expr, $out:expr) => {};
    (collect, $world:expr, $out:expr, $T:ty $(, $rest:ty)* $(,)?) => {
        collect_entities_with_component::<$T>($world, $out);
        component_helpers!(collect, $world, $out $(, $rest)*);
    };

    (baseline, $world:expr, $dest:expr, $entity:expr) => {};
    (baseline, $world:expr, $dest:expr, $entity:expr, $T:ty $(, $rest:ty)* $(,)?) => {
        write_component_baseline::<$T>($world, $dest, $entity);
        component_helpers!(baseline, $world, $dest, $entity $(, $rest)*);
    };

    (changes, $world:expr, $dest:expr, $newly_tracked:expr) => {};
    (changes, $world:expr, $dest:expr, $newly_tracked:expr, $T:ty $(, $rest:ty)* $(,)?) => {
        write_component_changes::<$T>($world, $dest, $newly_tracked);
        component_helpers!(changes, $world, $dest, $newly_tracked $(, $rest)*);
    };

    (removals, $world:expr, $dest:expr, $tracked:expr) => {};
    (removals, $world:expr, $dest:expr, $tracked:expr, $T:ty $(, $rest:ty)* $(,)?) => {
        write_component_removals::<$T>($world, $dest, $tracked);
        component_helpers!(removals, $world, $dest, $tracked $(, $rest)*);
    };
}

macro_rules! make_actual_enum {
    ($( $tag:expr, $T:tt ),* $(,)?) => {
        create_serialize_enum_simple!(
            ReplicatedComponentID,
            u8,
            {
                $( ($tag, $T), )*
            }
        );

        create_serialize_enum!(
            ReplicatedComponent,
            u8,
            {
                $( ($tag, $T, $T), )*
            }
        );

        create_serialize_enum_write_only!(
            ReplicatedComponentRef,
            u8,
            lifetime: 'b,
            {
                $( ($tag, $T, &'b $T), )*
            }
        );

        $(
            impl<'a> From<&'a $T> for ReplicatedComponentRef<'a> {
                fn from(v: &'a $T) -> Self {
                    Self::$T(v)
                }
            }
        )*

        trait IntoComponentID {
            const IDENTIFIER: ReplicatedComponentID;
        }

        impl ReplicatedComponentID {
            pub fn remove_component(self, e: Entity, commands: &mut Commands) {
                match self {
                    $( ReplicatedComponentID::$T => commands.entity(e).remove::<$T>(), )*
                };
            }
        }

        impl ReplicatedComponent {
            pub fn add_component(self, e: Entity, commands: &mut Commands) {
                match self {
                    $( ReplicatedComponent::$T(x) => commands.entity(e).insert(x), )*
                };
            }
        }

        $(
            impl IntoComponentID for $T {
                const IDENTIFIER: ReplicatedComponentID = ReplicatedComponentID::$T;
            }
        )*
    };
}

macro_rules! detect_component_changes {
    ($( ($id:expr, $type:tt) ),* $(,)?) => {
        make_actual_enum!( $( $id, $type ),* );

        pub(crate) fn collect_supported_component_entities(world: &mut World, out: &mut Vec<Entity>) {
            component_helpers!(collect, world, out, $( $type ),*);
        }

        pub(crate) fn write_component_baselines(
            world: &World,
            dest: &mut TranscriptWriteStateResource,
            entity: Entity,
        ) {
            component_helpers!(baseline, world, dest, entity, $( $type ),*);
        }

        pub(crate) fn write_changed_components(
            world: &mut World,
            dest: &mut TranscriptWriteStateResource,
            newly_tracked: &EntityHashSet,
        ) {
            component_helpers!(changes, world, dest, newly_tracked, $( $type ),*);
        }

        pub(crate) fn write_removed_components(
            world: &mut World,
            dest: &mut TranscriptWriteStateResource,
            tracked: &EntityHashSet,
        ) {
            component_helpers!(removals, world, dest, tracked, $( $type ),*);
        }
    }
}

#[derive(Resource)]
struct CachedRemovedComponents<C: Component> {
    state: SystemState<RemovedComponents<'static, 'static, C>>,
}

fn collect_entities_with_component<C: Component>(world: &mut World, out: &mut Vec<Entity>) {
    let mut query = world.query_filtered::<Entity, With<C>>();
    out.extend(query.iter(world));
}

fn write_component_baseline<C>(
    world: &World,
    dest: &mut TranscriptWriteStateResource,
    entity: Entity,
) where
    C: Component + FastWrite,
    for<'a> &'a C: Into<ReplicatedComponentRef<'a>>,
{
    let Some(component) = world.get::<C>(entity) else {
        return;
    };

    unsafe {
        ServerInstruction::CAdd(ServerComponentAdded {
            entity,
            component: component.into(),
        })
        .write_fast(dest);
    }
}

fn write_component_changes<C>(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    newly_tracked: &EntityHashSet,
) where
    C: Component + FastWrite,
    for<'a> &'a C: Into<ReplicatedComponentRef<'a>>,
{
    let mut query = world.query_filtered::<(Entity, &C), (Changed<C>, With<IsReplicated>)>();
    let mut items = Vec::new();

    for (entity, component) in query.iter(world) {
        if !newly_tracked.contains(&entity) {
            items.push((entity, component));
        }
    }

    for (entity, component) in items {
        unsafe {
            ServerInstruction::CAdd(ServerComponentAdded {
                entity,
                component: component.into(),
            })
            .write_fast(dest);
        }
    }
}

fn write_component_removals<C>(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    tracked: &EntityHashSet,
) where
    C: Component + IntoComponentID,
{
    if !world.contains_resource::<CachedRemovedComponents<C>>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedRemovedComponents::<C> { state });
    }

    world.resource_scope(|world, mut cached: Mut<CachedRemovedComponents<C>>| {
        let Ok(mut removals) = cached.state.get_mut(world) else {
            return;
        };
        for entity in removals.read() {
            if tracked.contains(&entity) {
                unsafe {
                    ServerInstruction::CRemove(ServerComponentRemoved {
                        entity,
                        component: C::IDENTIFIER,
                    })
                    .write_fast(dest);
                }
            }
        }
        cached.state.apply(world);
    });
}

type StandardMatComponent = MeshMaterial3d<StandardMaterial>;
type PointsMatComponent = MeshMaterial3d<PointsMaterial>;
type RLayers = bevy::camera::visibility::RenderLayers;

detect_component_changes!(
    (0, Head),
    (1, Transform),
    (2, Visibility),
    (3, PointLight),
    (4, DirectionalLight),
    (5, SpotLight),
    (6, Mesh3d),
    (7, StandardMatComponent),
    (8, PointsMatComponent),
    (9, InheritedVisibility),
    (10, NotShadowCaster),
    (11, NotShadowReceiver),
    (12, CascadeShadowConfig),
    (13, TextColor),
    (14, TextFont),
    (15, TextLayout),
    (16, TextSpan),
    (17, RLayers),
);
