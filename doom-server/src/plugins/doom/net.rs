use crate::plugins::doom::session::DoomSession;
use doom_protocol::{
    packet::{CmdBundle, TicCmd},
    MAX_PLAYERS,
};
use valence::prelude::*;

/// Plugin that allows doom sessions
/// to communicate with each other such that players
/// can play together.
pub struct DoomNetPlugin;

impl Plugin for DoomNetPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(DoomNetState::default())
        .add_systems(Update, (
            update_presence,
            collect_cmds,
            send_client_ticks,
        ));
    }
}

#[derive(Resource, Default)]
pub struct DoomNetState {
    pub next_tic: i32,
    pub present: [bool; MAX_PLAYERS],
    pub absent_ticks: [u8; MAX_PLAYERS],
    pub last_cmd: [TicCmd; MAX_PLAYERS],
    pub pending: [TicCmd; MAX_PLAYERS],
}

fn update_presence(mut net: ResMut<DoomNetState>, mut q: Query<&mut DoomSession>) {
    const GRACE: u8 = 35;
    let mut seen = [false; MAX_PLAYERS];

    for mut session in &mut q {
        let id = session.id().unsigned_abs() as usize - 1;

        if id < MAX_PLAYERS {
            session.set_local_player(id as i32);
            seen[id] = true;
        }
    }

    for (i, &seen_i) in seen.iter().enumerate() {
        if seen_i {
            net.present[i] = true;
            net.absent_ticks[i] = 0;
        } else if net.present[i] {
            net.absent_ticks[i] = net.absent_ticks[i].saturating_add(1);

            if net.absent_ticks[i] >= GRACE {
                net.present[i] = false;
            }
        }
    }
}

fn collect_cmds(mut net: ResMut<DoomNetState>, mut q: Query<&mut DoomSession>) {
    for session in &mut q {
        let player = (session.id().unsigned_abs() - 1) as usize;
        if player >= MAX_PLAYERS {
            continue;
        }

        let Ok(mut lock) = session.received_commands.try_lock() else {
            continue;
        };

        if lock.len() > 1 {
            println!(
                "Session {} running {} ticks ahead!",
                session.id(),
                lock.len()
            );
        }

        if let Some(mut cmd) = lock.pop_front() {
            cmd.player_id = player as i32;
            net.pending[player] = cmd;
        }
    }
}

fn send_client_ticks(mut q: Query<&mut DoomSession>, mut net: ResMut<DoomNetState>) {
    let t = net.next_tic;

    let mut bundle = CmdBundle {
        maketic: t,
        present: net.present,
        ..Default::default()
    };

    for p in 0..MAX_PLAYERS {
        bundle.cmds[p] = if net.present[p] {
            net.pending[p]
        } else {
            continue;
        }
    }

    for mut session in &mut q {
        session.send_cmd_bundle(bundle);
    }

    net.next_tic += 1;
}
