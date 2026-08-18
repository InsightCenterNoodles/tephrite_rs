use bevy::{
    asset::{AssetId, AssetPath},
    gltf::{Gltf, GltfAssetLabel, GltfMaterial, GltfMaterialName, GltfMeshName},
    platform::collections::HashMap,
    prelude::*,
    world_serialization::WorldAsset,
};

/// Retains root glTF handles while loading glTF scene subassets.
///
/// Loading `GltfAssetLabel::Scene` directly produces a [`WorldAsset`] scene,
/// but does not necessarily keep the root [`Gltf`] asset available. Tephrite's
/// lightweight glTF material bridge uses that root asset to map spawned glTF
/// material names back to their source [`GltfMaterial`] assets.
#[derive(Default, Resource)]
pub struct GltfSceneAssets {
    roots: Vec<Handle<Gltf>>,
}

impl GltfSceneAssets {
    /// Loads a glTF scene as a [`WorldAsset`] and retains the root [`Gltf`] asset.
    pub fn load_scene(
        &mut self,
        asset_server: &AssetServer,
        path: impl Into<AssetPath<'static>>,
        scene: usize,
    ) -> Handle<WorldAsset> {
        let path = path.into();
        self.roots.push(
            asset_server
                .load_builder()
                .override_unapproved()
                .load(path.clone()),
        );
        asset_server
            .load_builder()
            .override_unapproved()
            .load(GltfAssetLabel::Scene(scene).from_asset(path))
    }

    /// Retained root glTF handles.
    pub fn roots(&self) -> &[Handle<Gltf>] {
        &self.roots
    }
}

pub(crate) fn gltf_scene_assets_plugin(app: &mut App) {
    app.init_resource::<GltfSceneAssets>();
}

#[derive(Default, Resource)]
struct GltfStandardMaterialCache {
    by_gltf_material: HashMap<AssetId<GltfMaterial>, Handle<StandardMaterial>>,
    default_material: Option<Handle<StandardMaterial>>,
}

pub(crate) fn gltf_standard_material_bridge_plugin(app: &mut App) {
    app.init_resource::<GltfStandardMaterialCache>();
    app.add_systems(Update, assign_missing_gltf_standard_materials);
}

fn assign_missing_gltf_standard_materials(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<GltfStandardMaterialCache>,
    query: Query<
        (Entity, Option<&GltfMaterialName>),
        (
            With<GltfMeshName>,
            With<Mesh3d>,
            Without<MeshMaterial3d<StandardMaterial>>,
        ),
    >,
) {
    for (entity, material_name) in &query {
        let material = material_name
            .and_then(|name| find_gltf_material_by_name(&gltfs, name.as_ref()))
            .and_then(|handle| {
                material_from_gltf_handle(
                    handle,
                    &gltf_materials,
                    &mut standard_materials,
                    &mut cache,
                )
            })
            .unwrap_or_else(|| default_standard_material(&mut standard_materials, &mut cache));

        commands
            .entity(entity)
            .insert(MeshMaterial3d(material.clone()));
    }
}

fn find_gltf_material_by_name(gltfs: &Assets<Gltf>, name: &str) -> Option<Handle<GltfMaterial>> {
    gltfs
        .iter()
        .find_map(|(_, gltf)| gltf.named_materials.get(name).cloned())
}

fn material_from_gltf_handle(
    handle: Handle<GltfMaterial>,
    gltf_materials: &Assets<GltfMaterial>,
    standard_materials: &mut Assets<StandardMaterial>,
    cache: &mut GltfStandardMaterialCache,
) -> Option<Handle<StandardMaterial>> {
    if let Some(material) = cache.by_gltf_material.get(&handle.id()) {
        return Some(material.clone());
    }

    let gltf_material = gltf_materials.get(handle.id())?;
    let material = standard_materials.add(standard_material_from_gltf_material(gltf_material));
    cache.by_gltf_material.insert(handle.id(), material.clone());
    Some(material)
}

fn default_standard_material(
    standard_materials: &mut Assets<StandardMaterial>,
    cache: &mut GltfStandardMaterialCache,
) -> Handle<StandardMaterial> {
    cache
        .default_material
        .get_or_insert_with(|| {
            standard_materials.add(standard_material_from_gltf_material(
                &GltfMaterial::default(),
            ))
        })
        .clone()
}

fn standard_material_from_gltf_material(material: &GltfMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color: material.base_color,
        base_color_channel: material.base_color_channel.clone(),
        base_color_texture: material.base_color_texture.clone(),
        emissive: material.emissive,
        emissive_channel: material.emissive_channel.clone(),
        emissive_texture: material.emissive_texture.clone(),
        perceptual_roughness: material.perceptual_roughness,
        metallic: material.metallic,
        metallic_roughness_channel: material.metallic_roughness_channel.clone(),
        metallic_roughness_texture: material.metallic_roughness_texture.clone(),
        reflectance: material.reflectance,
        specular_tint: material.specular_tint,
        specular_transmission: material.specular_transmission,
        thickness: material.thickness,
        ior: material.ior,
        attenuation_distance: material.attenuation_distance,
        attenuation_color: material.attenuation_color,
        normal_map_channel: material.normal_map_channel.clone(),
        normal_map_texture: material.normal_map_texture.clone(),
        occlusion_channel: material.occlusion_channel.clone(),
        occlusion_texture: material.occlusion_texture.clone(),
        clearcoat: material.clearcoat,
        clearcoat_perceptual_roughness: material.clearcoat_perceptual_roughness,
        anisotropy_strength: material.anisotropy_strength,
        anisotropy_rotation: material.anisotropy_rotation,
        double_sided: material.double_sided,
        cull_mode: material.cull_mode,
        unlit: material.unlit,
        alpha_mode: material.alpha_mode,
        uv_transform: material.uv_transform,
        ..Default::default()
    }
}
