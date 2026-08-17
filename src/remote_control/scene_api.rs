use std::collections::HashMap;

use bevy::prelude::*;

pub(super) fn resolve_entity(world: &mut World, target: &str) -> Option<Entity> {
    parse_entity(target)
        .filter(|entity| world.get_entity(*entity).is_ok())
        .or_else(|| entity_by_exact_name(world, target))
}

fn parse_entity(target: &str) -> Option<Entity> {
    let bits = target
        .strip_prefix("Entity(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(target)
        .parse::<u64>()
        .ok()?;
    Some(Entity::from_bits(bits))
}

fn entity_by_exact_name(world: &mut World, name: &str) -> Option<Entity> {
    let mut query = world.query::<(Entity, &Name)>();
    query
        .iter(world)
        .find_map(|(entity, entity_name)| (entity_name.as_str() == name).then_some(entity))
}

pub(super) fn render_entities_json(world: &mut World, name_filter: Option<&str>) -> String {
    let mut query = world.query::<(Entity, Option<&Name>, Option<&Transform>)>();
    let mut out = String::from("{\"entities\":[");
    let mut first = true;

    for (entity, name, transform) in query.iter(world) {
        if let Some(filter) = name_filter {
            if name.map(Name::as_str) != Some(filter) {
                continue;
            }
        }

        if !first {
            out.push(',');
        }
        first = false;
        out.push_str("{\"id\":\"");
        out.push_str(&entity.to_bits().to_string());
        out.push_str("\",\"name\":");
        if let Some(name) = name {
            out.push('"');
            out.push_str(&json_escape(name.as_str()));
            out.push('"');
        } else {
            out.push_str("null");
        }
        out.push_str(",\"has_transform\":");
        out.push_str(if transform.is_some() { "true" } else { "false" });
        out.push('}');
    }

    out.push_str("]}");
    out
}

pub(super) fn render_transform_json(entity: Entity, transform: &Transform) -> String {
    let translation = transform.translation;
    let rotation = transform.rotation;
    let scale = transform.scale;
    format!(
        "{{\"entity\":\"{}\",\"translation\":[{},{},{}],\"rotation\":[{},{},{},{}],\"scale\":[{},{},{}]}}",
        entity.to_bits(),
        translation.x,
        translation.y,
        translation.z,
        rotation.x,
        rotation.y,
        rotation.z,
        rotation.w,
        scale.x,
        scale.y,
        scale.z
    )
}

pub(super) fn parse_vec3_fields(
    values: &HashMap<String, String>,
    x_key: &str,
    y_key: &str,
    z_key: &str,
) -> Option<Vec3> {
    let x = values.get(x_key)?.parse().ok()?;
    let y = values.get(y_key)?.parse().ok()?;
    let z = values.get(z_key)?.parse().ok()?;
    Some(Vec3::new(x, y, z))
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}
