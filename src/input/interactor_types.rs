// WIP

pub struct Controller<'a>(&'a super::InteractorState);

#[derive(Debug, Default, Clone, Copy)]
pub enum ControllerJoystickAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    DPad,
    #[default]
    Unknown,
}

// impl InteractorState {
//     fn get_axis_value(&self, axis: ControllerJoystickAxis) -> Option<f32> {
//         self.analogs.get(axis as usize).cloned().flatten()
//     }

//     fn set_axis_value(&mut self, axis: ControllerJoystickAxis, value: Option<f32>) {
//         if let Some(v) = self.analogs.get_mut(axis as usize) {
//             *v = value;
//         }
//     }

//     pub fn stick_state(&self, stick: JoystickID) -> Option<Vec2> {
//         let (a, b) = match stick {
//             JoystickID::Joystick0 => (JoystickAxis::LeftX, JoystickAxis::LeftY),
//             JoystickID::Joystick1 => (JoystickAxis::RightX, JoystickAxis::RightY),
//             JoystickID::Joystick2 => (JoystickAxis::DPad, JoystickAxis::DPad),
//         };

//         // a stick is valid if either one of its axis is not null

//         match (self.get_axis_value(a), self.get_axis_value(b)) {
//             (None, None) => None,
//             (None, Some(y)) => Some(vec2(0.0, y)),
//             (Some(x), None) => Some(vec2(x, 0.0)),
//             (Some(x), Some(y)) => Some(vec2(x, y)),
//         }
//     }
// }

pub struct DTrackFlystick<'a>(&'a super::InteractorState);
