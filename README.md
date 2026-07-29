# Teprite (`tephrite_rs`)

Teprite is a Rust-based immersive visualization renderer built on top of Bevy. It’s designed for multi-display / CAVE-style rendering by running your Bevy app as a **logic process** that spawns one or more **render processes**. World state is replicated from the logic process to render processes, and render processes present the scene to the configured screens.

- Platforms: macOS + Linux (Windows is WIP)
- Render backends: Vulkan / Metal 
- Tracking + input: optional VRPN head tracking and joystick/button events

NRL SWR: SWR 26-061

## How to use (quick start)

Tephrite is built off of [bevy]("bevyengine.org"). You can build your app as usual, and then adopt Teprite’s rendering architecture. To do so:

1. Use a supported version of Bevy in your Cargo.toml: 
```toml 
bevy = "0.19.0"
```
2. Ensure that you are patching Bevy:
```toml
[patch.crates-io]
bevy = { git = "https://github.com/nicholasbl/bevy", branch = "cave_patches" }
```
3. Structure your app to start via a single plugin.

4. Call `tephrite_rs::run(YourPlugin)` (not `App::run()` directly). 

5. Make sure you have a valid configuration for your immersive setup. If not, your app will be in simulator mode.

6. Run!

## Minimal code structure

The examples show the intended pattern: define a Bevy plugin, then call `tephrite_rs::run(MyPlugin)`.

```rust
use bevy::prelude::*;
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

impl TephriteApp for MyPlugin {}

fn setup(mut commands: Commands) {
    commands.spawn((DirectionalLight::default(), Transform::default()));
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
```

## Architecture

- **Logic process**
  - Builds a normal Bevy `App` and adds your plugin(s)
  - Spawns N render processes (N = number of `[[screens]]` entries in the config)
  - Writes replication data (entities/components/assets/resources) to shared memory
  - Optionally consumes VRPN and produces “head” + joystick/button events
- **Render process(es)**
  - Starts renderer and opens a window for its assigned `[[screen]]`
  - Reads replication data and reconstructs the replicated world
  - Renders the replicated scene

### Replication basics

Replication is automatic for entities with supported render-side components. Parent
entities are automatically tracked to the root so transform hierarchy is preserved.

The prelude exports the most commonly used pieces:
- `tephrite_rs::prelude::run`
- input utilities (navigator + interactors)
- `Head`, `EnvironmentLighting`

## Configuration

Teprite loads a TOML configuration file at startup.

### Config file discovery

Search order:

1. `$TEPH_CONFIG_PATH`
2. `~/.teph/config.toml`
3. `~/.config/teph.toml`
4. `/opt/teph/config.toml`
5. `/etc/teph/config.toml`

You can start from `assets/config_example.toml`.

### Top-level fields

```toml
use_offaxis = true     # enable immersive mode (simulator mode otherwise)
debug_renderer = false # enable renderer-side debug logging

[render]
api = "vulkan"                 # one of: "vulkan", "metal", "opengl"

[vrpn]
head = "Head0/0@127.0.0.1:3883"  # optional; sensor is optional and defaults to 0
joystick = "Joy0@127.0.0.1:3883,Joy1/1@127.0.0.1:3883"  # optional, comma-separated
coordinate_transform = "vrpn_bevy"  # optional: "vrpn_bevy" (default) or "identity"
```

Notes:
- `use_offaxis = false` is appropriate for local single-display development.
- `debug_renderer` controls render-process logging (the logic process also uses it to pass a debug env var to children).
- VRPN addresses are parsed as `sender@host:port` or `sender/sensor@host:port`.
- `coordinate_transform = "vrpn_bevy"` preserves Tephrite's historical VRPN mapping; use `"identity"` when the VRPN server already reports coordinates in Tephrite/Bevy space.

### `[[displays]]`: physical screens in room coordinates

Each `[[display]]` describes a *physical* display plane in 3D space (room coordinates) plus its pixel resolution. If using VRPN, ensure that these coordinates are the same as the tracker coordinates.

```toml
[[displays]]
lower_left  = [-1.0, 0.0, 0.0]
lower_right = [ 1.0, 0.0, 0.0]
upper_right = [ 1.0, 1.0, 0.0]
```

Semantics:
- The three corners define the display plane and orientation.

### `[[screens]]`: windows (render processes) bound to displays

Each `[[screen]]` corresponds to **one spawned render process**. The Nth entry in `[[screens]]` is assigned to child process rank N.

```toml
[[screens]]
display = 0          # index into [[displays]]
card_index = 0       # optional GPU device index (backend-dependent)
x_display = ":0.0"   # optional X11 display string (Linux)
fullscreen = false   # optional (default false)
is_right = false     # optional (default false); stereo eye selection when use_offaxis=true
```

Notes:
- If you want a single window, define exactly one `[[screens]]` entry.
- `is_right` only matters when `use_offaxis = true`; it selects left/right eye parameters for stereo setups.
- `x_display` is primarily for multi-X-screen Linux setups.

### Minimal “single window” config (local dev)

```toml
use_offaxis = false
debug_renderer = false

[[displays]]
lower_left  = [-1.0, 0.0, 0.0]
lower_right = [ 1.0, 0.0, 0.0]
upper_right = [ 1.0, 1.0, 0.0]
resolution = [1280, 720]

[[screens]]
display = 0
fullscreen = false
```

## VRPN (head + joystick)

VRPN is optional:

- If `[vrpn].head` is set, Teprite spawns a `Head` entity and updates its transform from VRPN.
- If `[vrpn].joystick` is set, Teprite spawns an `Interactor` entity and emits button/axis messages from VRPN input.

## Examples

- Starter: `cargo run --example mesh`
- Also included: `basic_animation`, `load_mesh`, `image_based_lighting`

## Testing

Use `cargo test -- --test-threads=1` to avoid some tests deadlocking.

## License

MIT. See `LICENSE`.
