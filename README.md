# Teprite (`tephrite_rs`)

Teprite is a Rust-based immersive visualization renderer built on top of Bevy. It’s designed for multi-display / CAVE-style rendering by running your Bevy app as a **logic process** that spawns one or more **render processes**. World state is replicated from the logic process to render processes, and render processes present the scene using the native `libbackfill` renderer.

- Platforms: macOS + Linux (Windows is WIP)
- Render backends: Vulkan / Metal / OpenGL (configurable)
- Tracking + input: optional VRPN head tracking and joystick/button events

## How to use (quick start)

1. Install `libbackfill` (unless your machine already has it installed globally):
   - https://github.com/InsightCenterNoodles/libbackfill
2. Create a Teprite config file (see “Configuration” below).
3. Run the starter example:
   - `cargo run --example mesh`

Teprite applications should call `tephrite_rs::run(...)` (not `App::run()` directly). `run()` decides whether the current process is the logic process or a spawned render child process.

## Architecture

- **Logic process**
  - Builds a normal Bevy `App` and adds your plugin(s)
  - Spawns N render processes (N = number of `[[screens]]` entries in the config)
  - Writes replication data (entities/components/assets/resources) to shared memory
  - Optionally consumes VRPN and produces “head” + joystick/button events
- **Render process(es)**
  - Starts `libbackfill` and opens a window for its assigned `[[screen]]`
  - Reads replication data and reconstructs the replicated world
  - Renders the replicated scene

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

fn setup(mut commands: Commands) {
    commands.spawn((DirectionalLight::default(), Transform::default(), Replicated));
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
```

### Replication basics

Replication is opt-in: mark what should appear in the render process.

- `Replicated`: tag an entity to replicate it
- `PropagateReplication`: propagate replication through hierarchy (parent/children)

The prelude exports the most commonly used pieces:
- `tephrite_rs::prelude::run`
- `Replicated`, `PropagateReplication`
- input utilities (navigator + interactors)
- `Head`, `EnvironmentLighting`

## Native dependency: `libbackfill`

Teprite links against `libbackfill` (ABI version `backfill-0`). Bevy does not yet support offaxis projections, and is still under development. This dependency provides a stable render layer through Google's Filament, and
will be removed once Bevy's renderer better supports our use case.

### Option A: global install or local install in known location (recommended when available)

If `libbackfill` is installed system-wide and exposes a pkg-config file for `backfill-0`, builds should "just work".

If the library is installed in `~/.local`, builds should also "just work".

### Option B: custom install path

If `pkg-config` can’t find `backfill-0`, set one of:

- `BACKFILL_DIR` (prefix containing `include/` and `lib/`)
- `BACKFILL_INCLUDE_DIR` (directory containing the `backfill-0/.../api.h` header)
- `BACKFILL_LIB_DIR` (directory containing `libbackfill-0.(dylib|so)`)

If you see an error like:
`Could not find headers or library for backfill-0`
it means Teprite couldn’t locate both the headers and the shared library.

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
use_offaxis = false            # enable off-axis stereo projection (CAVE-style)
debug_renderer = false         # enable renderer-side debug logging

[render]
api = "vulkan"                 # one of: "vulkan", "metal", "opengl"

[vrpn]
head = "Head0@127.0.0.1:3883"  # optional
joystick = "Joy0@127.0.0.1:3883,Joy1@127.0.0.1:3883"  # optional, comma-separated
debug_head = false             # if true, synthesize head motion when no head tracker is configured
```

Notes:
- `use_offaxis = false` is appropriate for local single-display development.
- `debug_renderer` controls render-process logging (the logic process also uses it to pass a debug env var to children).
- VRPN addresses are parsed as `sender@host:port`.

### `[[displays]]`: physical screens in room coordinates

Each `[[display]]` describes a *physical* display plane in 3D space (room coordinates) plus its pixel resolution.

```toml
[[displays]]
lower_left  = [-1.0, 0.0, 0.0]
lower_right = [ 1.0, 0.0, 0.0]
upper_right = [ 1.0, 1.0, 0.0]
resolution = [1920, 1200]
```

Semantics:
- The three corners define the display plane and orientation.
- `resolution` is `[width, height]` in pixels.

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

- If `[vrpn].head` is set, Teprite spawns a replicated `Head` entity and updates its transform from VRPN.
- If `[vrpn].debug_head = true` and no head is configured, Teprite synthesizes head motion for debugging.
- If `[vrpn].joystick` is set, Teprite spawns an `Interactor` entity and emits button/axis messages from VRPN input.

## Examples

- Starter: `cargo run --example mesh`
- Also included: `basic_animation`, `load_mesh`, `image_based_lighting`

## Testing

Use `cargo test -- --test-threads=1` to avoid some tests deadlocking.

## License

MIT. See `LICENSE`.

