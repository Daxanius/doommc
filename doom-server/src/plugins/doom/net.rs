use doom_protocol::{packet::CmdBundle, MAX_PLAYERS};
use valence::prelude::*;

use crate::plugins::doom::session::DoomSession;

/// Plugin that allows doom sessions
/// to communicate with each other such that players
/// can play together.
pub struct DoomNetPlugin;

impl Plugin for DoomNetPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(DoomNetState::default())
        .add_systems(
            Update,
            (
                update_presence,
                collect_latest_cmds,
                send_client_ticks,
            ),
        );
    }
}

#[derive(Resource, Default)]
pub struct DoomNetState {
    pub bundle: CmdBundle,
    pub absent_ticks: [u8; MAX_PLAYERS],
}

fn update_presence(mut net: ResMut<DoomNetState>, mut q: Query<&mut DoomSession>) {
    const GRACE: u8 = 100;
    let mut seen = [false; MAX_PLAYERS];

    for mut session in &mut q {
        let id = session.id().unsigned_abs() as usize - 1;
        if id < MAX_PLAYERS {
            session.set_local_player(id as i32);
            seen[id] = true;
        }
    }

    for (i, &is_seen) in seen.iter().enumerate() {
        if is_seen {
            net.bundle.present[i] = true;
            net.absent_ticks[i] = 0;
        } else if net.bundle.present[i] {
            net.absent_ticks[i] = net.absent_ticks[i].saturating_add(1);
            if net.absent_ticks[i] >= GRACE {
                net.bundle.present[i] = false;
            }
        }
    }
}

fn collect_latest_cmds(mut net: ResMut<DoomNetState>, mut q: Query<&mut DoomSession>) {
    for session in &mut q {
        let player = session.id().unsigned_abs() - 1;
        if player >= MAX_PLAYERS.try_into().unwrap() {
            continue;
        }

        let Ok(mut lock) = session.received_command.try_lock() else {
            continue;
        };

        if let Some(mut cmd) = lock.take() {
            cmd.player_id = player.cast_signed();
            net.bundle.cmds[player as usize] = cmd;
        }
    }
}

fn send_client_ticks(mut q: Query<&mut DoomSession>, mut net: ResMut<DoomNetState>) {
    for mut session in &mut q {
        session.send_cmd_bundle(net.bundle);
    }

    net.bundle.maketic += 1;
}
