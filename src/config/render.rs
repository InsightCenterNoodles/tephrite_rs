use bevy::{
    math::{DVec3, UVec2, uvec2},
    reflect::Reflect,
};

use super::{Config, Display, InteractorConfig, InteractorType, Screen};

/// Physical location of the display, as measured in room coordinates
#[derive(Debug, Default, Reflect, Clone)]
pub struct DisplayPhysical {
    pub lower_left: DVec3,
    pub lower_right: DVec3,
    pub upper_right: DVec3,
}

impl DisplayPhysical {
    fn make_plain() -> Self {
        Self {
            lower_left: [-1.0, 0.0, 0.0].into(),
            lower_right: [1.0, 0.0, 0.0].into(),
            upper_right: [1.0, 1.0, 0.0].into(),
        }
    }
}

impl Display {
    pub fn physical(&self) -> DisplayPhysical {
        DisplayPhysical {
            lower_left: self.lower_left.into(),
            lower_right: self.lower_right.into(),
            upper_right: self.upper_right.into(),
        }
    }
}

impl Config {
    pub fn child_count(&self) -> u32 {
        self.screens.len().try_into().unwrap_or(u32::MAX)
    }

    pub fn interactor(&self) -> Option<InteractorConfig> {
        self.vrpn.interactor.clone().or_else(|| {
            self.vrpn
                .joystick_legacy
                .clone()
                .map(|addresses| InteractorConfig {
                    addresses,
                    ty: InteractorType::Controller,
                })
        })
    }

    pub fn screen_for_rank(&self, rank: u32) -> Option<&Screen> {
        self.screens.get(rank as usize)
    }

    pub fn display_for_screen(&self, screen: &Screen) -> Option<&Display> {
        self.displays.get(screen.display as usize)
    }

    pub fn render_configuration(&self, rank: u32) -> RenderConfiguration {
        let this_screen = self.screen_for_rank(rank);
        let this_display = this_screen.and_then(|screen| self.display_for_screen(screen));

        let resolution = this_screen
            .and_then(|screen| screen.placement.as_ref())
            .map(|placement| placement.resolution.into())
            .or_else(|| this_display.map(|display| display.resolution.into()))
            .unwrap_or_else(|| uvec2(1920, 1200));

        let placement = this_screen
            .and_then(|screen| screen.placement.as_ref())
            .map(|placement| placement.location.into())
            .unwrap_or_else(|| uvec2(0, 0));

        let mono_override = std::env::var("TEPH_MONO").ok().map(|_| false);

        RenderConfiguration {
            use_offaxis: self.use_offaxis,
            debug_renderer: self.debug_renderer,
            process_rank: rank,
            card_index: this_screen.and_then(|screen| screen.card_index),
            fullscreen: this_screen
                .map(|screen| screen.fullscreen)
                .unwrap_or_default(),
            is_right: mono_override
                .or_else(|| this_screen.map(|screen| screen.is_right))
                .unwrap_or_default(),
            display_name: this_screen.and_then(|screen| screen.x_display.clone()),
            display_physical: this_display
                .map(Display::physical)
                .unwrap_or_else(DisplayPhysical::make_plain),
            resolution,
            placement,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RenderConfiguration {
    pub use_offaxis: bool,

    pub debug_renderer: bool,

    /// The rank of the process
    pub process_rank: u32,

    /// The display to use
    pub display_name: Option<String>,

    /// The graphics card to use
    pub card_index: Option<u32>,

    /// The physical disposition of the display
    pub display_physical: DisplayPhysical,

    /// The pixel resolution of the display (w, h)
    pub resolution: UVec2,

    pub placement: UVec2,

    pub fullscreen: bool,

    pub is_right: bool,
}
