use bevy::prelude::*;

use crate::input::{InputButton, InteractorAction};

pub trait InteractorTrait {
    type Stick: Copy;
    type Button: Copy;

    fn decay() -> &'static [usize];

    fn stick_state(stick: Self::Stick, state: &super::InteractorState) -> Option<Vec2>;

    fn translate_button(button: Self::Button) -> Option<InputButton>;

    fn reverse_translate_button(button: InputButton) -> Option<Self::Button>;

    fn action_for_button(button: InputButton) -> Option<InteractorAction>;
    fn button_for_action(action: InteractorAction) -> Option<InputButton>;

    fn pressed(button: Self::Button, state: &super::InteractorState) -> bool {
        Self::translate_button(button)
            .map(|x| state.buttons.pressed(x))
            .unwrap_or_default()
    }

    fn just_pressed(button: Self::Button, state: &super::InteractorState) -> bool {
        Self::translate_button(button)
            .map(|x| state.buttons.just_pressed(x))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ControllerStick {
    Left,
    Right,
    DPad,
}

pub struct Controller;

const LEFT_X_AXIS: usize = 0;
const LEFT_Y_AXIS: usize = 1;
const RIGHT_X_AXIS: usize = 2;
const RIGHT_Y_AXIS: usize = 5;
const DPAD_AXIS: usize = 8;

impl InteractorTrait for Controller {
    type Stick = ControllerStick;
    type Button = ControllerButton;

    fn decay() -> &'static [usize] {
        static DEC_VALS: [usize; 4] = [LEFT_X_AXIS, LEFT_Y_AXIS, RIGHT_X_AXIS, RIGHT_Y_AXIS];

        return &DEC_VALS;
    }

    fn stick_state(stick: Self::Stick, state: &super::InteractorState) -> Option<Vec2> {
        let (a, b) = match stick {
            ControllerStick::Left => (LEFT_X_AXIS, LEFT_Y_AXIS),
            ControllerStick::Right => (RIGHT_X_AXIS, RIGHT_Y_AXIS),
            ControllerStick::DPad => (DPAD_AXIS, DPAD_AXIS),
        };

        // a stick is valid if either one of its axis is not null

        match (state.get_axis_value(a), state.get_axis_value(b)) {
            (None, None) => None,
            (None, Some(y)) => Some(vec2(0.0, y)),
            (Some(x), None) => Some(vec2(x, 0.0)),
            (Some(x), Some(y)) => Some(vec2(x, y)),
        }
    }

    fn translate_button(button: Self::Button) -> Option<InputButton> {
        match button {
            ControllerButton::X => Some(InputButton::Button0),
            ControllerButton::A => Some(InputButton::Button1),
            ControllerButton::B => Some(InputButton::Button2),
            ControllerButton::Y => Some(InputButton::Button3),
            ControllerButton::BL => Some(InputButton::Button4),
            ControllerButton::BR => Some(InputButton::Button5),
            ControllerButton::TL => Some(InputButton::Button6),
            ControllerButton::TR => Some(InputButton::Button7),
            ControllerButton::Back => Some(InputButton::Button8),
            ControllerButton::Start => Some(InputButton::Button9),
            ControllerButton::Unknown => None,
        }
    }

    fn reverse_translate_button(button: InputButton) -> Option<Self::Button> {
        match button {
            InputButton::Button0 => Some(ControllerButton::X),
            InputButton::Button1 => Some(ControllerButton::A),
            InputButton::Button2 => Some(ControllerButton::B),
            InputButton::Button3 => Some(ControllerButton::Y),
            InputButton::Button4 => Some(ControllerButton::BL),
            InputButton::Button5 => Some(ControllerButton::BR),
            InputButton::Button6 => Some(ControllerButton::TL),
            InputButton::Button7 => Some(ControllerButton::TR),
            InputButton::Button8 => Some(ControllerButton::Back),
            InputButton::Button9 => Some(ControllerButton::Start),
            _ => None,
        }
    }

    fn action_for_button(button: InputButton) -> Option<InteractorAction> {
        match Self::reverse_translate_button(button)? {
            ControllerButton::A => Some(InteractorAction::Primary),
            ControllerButton::B => Some(InteractorAction::Secondary),
            ControllerButton::Y => Some(InteractorAction::Menu),
            ControllerButton::Start => Some(InteractorAction::ResetView),
            ControllerButton::TL => Some(InteractorAction::Previous),
            ControllerButton::TR => Some(InteractorAction::Next),
            _ => None,
        }
    }

    fn button_for_action(action: InteractorAction) -> Option<InputButton> {
        let b = match action {
            InteractorAction::Primary => ControllerButton::A,
            InteractorAction::Secondary => ControllerButton::B,
            InteractorAction::Menu => ControllerButton::Y,
            InteractorAction::ResetView => ControllerButton::Start,
            InteractorAction::Previous => ControllerButton::TL,
            InteractorAction::Next => ControllerButton::TR,
        };
        Self::translate_button(b)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerButton {
    X,
    Y,
    A,
    B,
    BL,
    BR,
    TL,
    TR,
    Back,
    Start,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum FlystickStick {
    Stick,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlystickButton {
    Trigger,
    RedButton,
    BlackButton,
    BlueButton,
    GreyButton,
    JoystickButton,
    LeftWhiteButton,
    RightWhiteButton,
    #[default]
    Unknown,
}

pub struct DTrackFlystick;

const FLYSTICK_X_AXIS: usize = 0;
const FLYSTICK_Y_AXIS: usize = 1;

impl InteractorTrait for DTrackFlystick {
    type Stick = FlystickStick;
    type Button = FlystickButton;

    fn decay() -> &'static [usize] {
        static DEC_VALS: [usize; 2] = [FLYSTICK_X_AXIS, FLYSTICK_Y_AXIS];

        &DEC_VALS
    }

    fn stick_state(stick: Self::Stick, state: &super::InteractorState) -> Option<Vec2> {
        let (a, b) = match stick {
            FlystickStick::Stick => (FLYSTICK_X_AXIS, FLYSTICK_Y_AXIS),
        };

        match (state.get_axis_value(a), state.get_axis_value(b)) {
            (None, None) => None,
            (None, Some(y)) => Some(vec2(0.0, y)),
            (Some(x), None) => Some(vec2(x, 0.0)),
            (Some(x), Some(y)) => Some(vec2(x, y)),
        }
    }

    fn translate_button(button: Self::Button) -> Option<InputButton> {
        match button {
            FlystickButton::Trigger => Some(InputButton::Button0),
            FlystickButton::RedButton => Some(InputButton::Button1),
            FlystickButton::BlackButton => Some(InputButton::Button2),
            FlystickButton::BlueButton => Some(InputButton::Button3),
            FlystickButton::GreyButton => Some(InputButton::Button4),
            FlystickButton::JoystickButton => Some(InputButton::Button5),
            FlystickButton::LeftWhiteButton => Some(InputButton::Button6),
            FlystickButton::RightWhiteButton => Some(InputButton::Button7),
            FlystickButton::Unknown => None,
        }
    }

    fn reverse_translate_button(button: InputButton) -> Option<Self::Button> {
        match button {
            InputButton::Button0 => Some(FlystickButton::Trigger),
            InputButton::Button1 => Some(FlystickButton::RedButton),
            InputButton::Button2 => Some(FlystickButton::BlackButton),
            InputButton::Button3 => Some(FlystickButton::BlueButton),
            InputButton::Button4 => Some(FlystickButton::GreyButton),
            InputButton::Button5 => Some(FlystickButton::JoystickButton),
            InputButton::Button6 => Some(FlystickButton::LeftWhiteButton),
            InputButton::Button7 => Some(FlystickButton::RightWhiteButton),
            _ => None,
        }
    }

    fn action_for_button(button: InputButton) -> Option<InteractorAction> {
        match Self::reverse_translate_button(button)? {
            FlystickButton::Trigger => Some(InteractorAction::Primary),
            FlystickButton::RedButton => Some(InteractorAction::Secondary),
            FlystickButton::BlackButton => Some(InteractorAction::Menu),
            FlystickButton::JoystickButton => Some(InteractorAction::ResetView),
            _ => None,
        }
    }

    fn button_for_action(action: InteractorAction) -> Option<InputButton> {
        let b = match action {
            InteractorAction::Primary => FlystickButton::Trigger,
            InteractorAction::Secondary => FlystickButton::RedButton,
            InteractorAction::Menu => FlystickButton::BlackButton,
            InteractorAction::ResetView => FlystickButton::JoystickButton,
            _ => return None,
        };
        Self::translate_button(b)
    }
}
