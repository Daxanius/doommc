use doom_protocol::packet::Input;
use doom_protocol::util::float_to_delta;
use valence::interact_item::InteractItemEvent;
use valence::math::Vec3Swizzles;
use valence::movement::MovementEvent;
use valence::prelude::*;

use crate::plugins::doom::session::DoomSession;
use crate::plugins::doom::util::{should_be_active, wrap_degrees};
use crate::plugins::hotbar::SelectedHotbarSlot;

pub struct DoomInputPlugin;

/// Plugin that handles user input when
/// the user is holding a map corresponding
/// to their own doom session ID.
impl Plugin for DoomInputPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .add_systems(
            Update,
            (
                freeze_controllers,
                handle_controller_sneak,
                handle_controller_move,
                handle_controller_stop,
                handle_controller_interact,
                update_session_active_from_held_item,
            ),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveState {
    #[default]
    None,
    Forward,
    Back,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrafeState {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Component)]
pub struct DoomController {
    pub last_tick: i64,
    pub last_yaw: f32,
    pub move_state: MoveState,
    pub strafe_state: StrafeState,
    pub freeze_position: DVec3,
    pub correction: DVec3, // previous reference
}

impl Default for DoomController {
    fn default() -> Self {
        Self {
            last_tick: 0,
            last_yaw: 0.0,
            move_state: MoveState::None,
            strafe_state: StrafeState::None,
            freeze_position: DVec3::ZERO,
            correction: DVec3::ZERO,
        }
    }
}

impl DoomController {
    #[must_use]
    pub fn new(freeze_position: DVec3) -> Self {
        Self {
            freeze_position,
            ..Default::default()
        }
    }
}

fn freeze_controllers(mut q: Query<(&mut Position, &mut DoomController)>) {
    const DEADZONE: f64 = 0.35;

    for (mut position, mut controller) in &mut q {
        let cur = position.0;
        let target = controller.freeze_position;

        let delta = cur - target;
        let dist = delta.length();

        if dist > DEADZONE {
            let new_pos = controller.freeze_position;
            controller.correction = new_pos - position.0;
            position.0 = new_pos;
        }
    }
}

fn handle_controller_stop(server: Res<Server>, mut q: Query<(&mut DoomSession, &DoomController)>) {
    let current_tick = server.current_tick();

    for (mut session, controller) in &mut q {
        if current_tick > controller.last_tick + 10 {
            session.set_input(Input::Up, false);
            session.set_input(Input::Down, false);
            session.set_input(Input::Left, false);
            session.set_input(Input::Right, false);
        }
    }
}

fn handle_controller_move(
    mut ev: EventReader<MovementEvent>,
    server: Res<Server>,
    mut q: Query<(&mut DoomSession, &mut DoomController)>,
) {
    const MOVE_ON: f64 = 0.03;
    const MOVE_OFF: f64 = 0.015;

    for e in ev.read() {
        let Ok((mut session, mut controller)) = q.get_mut(e.client) else {
            continue;
        };

        let delta = (e.position - e.old_position) - controller.correction;
        controller.correction = DVec3::ZERO;

        let yaw = controller.last_yaw.to_radians();
        let forward = Vec2::new(-yaw.sin(), yaw.cos()).normalize();
        let right = -Vec2::new(yaw.cos(), yaw.sin()).normalize();

        let f = delta.xz().dot(forward.as_dvec2());
        let r = delta.xz().dot(right.as_dvec2());

        let current_tick = server.current_tick();

        // turn from yaw delta
        let dyaw = wrap_degrees(e.look.yaw - controller.last_yaw);
        if (controller.last_yaw - dyaw).abs() > f32::EPSILON {
            session.set_mouse_delta(float_to_delta(dyaw));
        }

        controller.last_yaw = e.look.yaw;
        controller.last_tick = current_tick;

        let forward_on = if controller.move_state == MoveState::Forward {
            f > MOVE_OFF
        } else {
            f > MOVE_ON
        };

        let back_on = if controller.move_state == MoveState::Back {
            f < -MOVE_OFF
        } else {
            f < -MOVE_ON
        };

        controller.move_state = if forward_on {
            MoveState::Forward
        } else if back_on {
            MoveState::Back
        } else {
            MoveState::None
        };

        session.set_input(Input::Up, forward_on);
        session.set_input(Input::Down, back_on);

        let strafe_r = if controller.strafe_state == StrafeState::Right {
            r > MOVE_OFF
        } else {
            r > MOVE_ON
        };
        let strafe_l = if controller.strafe_state == StrafeState::Left {
            r < -MOVE_OFF
        } else {
            r < -MOVE_ON
        };

        controller.strafe_state = if strafe_r {
            StrafeState::Right
        } else if strafe_l {
            StrafeState::Left
        } else {
            StrafeState::None
        };

        session.set_input(Input::StrafeRight, strafe_r);
        session.set_input(Input::StrafeLeft, strafe_l);
    }
}

fn handle_controller_sneak(mut ev: EventReader<SneakEvent>, mut q: Query<&mut DoomSession>) {
    for e in &mut ev.read() {
        let Ok(mut session) = q.get_mut(e.client) else {
            continue;
        };

        let down = e.state == SneakState::Start;
        session.set_input(Input::Shoot, down);
        session.set_input(Input::Enter, down);
    }
}

fn handle_controller_interact(
    mut ev: EventReader<InteractItemEvent>,
    mut q: Query<&mut DoomSession>,
) {
    for e in &mut ev.read() {
        let Ok(mut session) = q.get_mut(e.client) else {
            continue;
        };

        session.toggle_input(Input::Use);
    }
}

fn update_session_active_from_held_item(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut DoomSession,
        &Inventory,
        &Position,
        &Look,
        &SelectedHotbarSlot,
    )>,
) {
    for (player, mut session, inventory, position, look, selected) in &mut q {
        let active = should_be_active(session.id(), inventory, selected.0);

        if session.set_active(active) {
            if active {
                commands.entity(player).insert(DoomController {
                    freeze_position: position.0,
                    last_yaw: look.yaw,
                    ..Default::default()
                });
            } else {
                commands.entity(player).remove::<DoomController>();
            }
        }
    }
}
