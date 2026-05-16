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
        .add_systems(Update, (
            party_presence,
            collect_cmds,
            send_client_ticks,
        ));
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DoomPartyId(pub Uuid);

#[derive(Component, Default)]
pub struct DoomTickStream {
    pub next_tic: i32,
    pub present: [bool; MAX_PLAYERS],
    pub last_cmd: [TicCmd; MAX_PLAYERS],
    pub pending: [TicCmd; MAX_PLAYERS],
}

pub struct DoomPartyInvite {
    pub target: Entity,
    pub created_tick: i64,
}

#[derive(Component)]
pub struct DoomParty {
    pub wad: String,
    pub max_players: usize,
    pub started: bool,
    pub host: Entity, // who can /doom start
    pub invites: Vec<DoomPartyInvite>,
}

impl DoomParty {
    #[must_use]
    pub fn is_full(&self, slot: usize) -> bool {
        slot >= self.max_players.min(MAX_PLAYERS)
    }

    pub fn start(
        &mut self,
        party_ent: Entity,
        net: &mut DoomTickStream,
        members: &Query<(Entity, &DoomPartyMember)>,
        sessions: &mut Query<&mut DoomSession>,
    ) -> [bool; MAX_PLAYERS] {
        self.started = true;
        net.next_tic = 0;

        let mut present = [false; MAX_PLAYERS];
        let member_list = Self::list_members(party_ent, members);

        for (_e, slot) in &member_list {
            present[*slot] = true;
        }
        net.present = present;

        for (player_ent, slot) in member_list {
            if let Ok(mut session) = sessions.get_mut(player_ent) {
                session.set_local_player(slot as i32);
                for s in 0..MAX_PLAYERS {
                    if present[s] {
                        session.add_player(s as i32);
                    }
                }
            }
        }

        present
    }

    pub fn list_members(
        party_ent: Entity,
        members: &Query<(Entity, &DoomPartyMember)>,
    ) -> Vec<(Entity, usize)> {
        members
            .iter()
            .filter(|(_, m)| m.group == party_ent && m.slot < MAX_PLAYERS)
            .map(|(e, m)| (e, m.slot))
            .collect()
    }
}

/// An entity inside of a doom group
#[derive(Component)]
pub struct DoomPartyMember {
    pub group: Entity,
    pub slot: usize,
}

fn party_presence(
    mut groups: Query<(Entity, &mut DoomTickStream, &DoomParty)>,
    members: Query<&DoomPartyMember>,
) {
    for (group_ent, mut net, _group) in &mut groups {
        let mut present = [false; MAX_PLAYERS];

        for m in &members {
            if m.group == group_ent && m.slot < MAX_PLAYERS {
                present[m.slot] = true;
            }
        }

        net.present = present;
    }
}

fn collect_cmds(
    mut groups: Query<(Entity, &mut DoomTickStream, &DoomParty)>,
    mut players: Query<(Entity, &DoomSession, &DoomPartyMember)>,
) {
    for (group_ent, mut net, group) in &mut groups {
        if !group.started {
            continue;
        }

        for (_player_ent, session, m) in &mut players {
            if m.group != group_ent {
                continue;
            }

            let Ok(mut lock) = session.received_commands.try_lock() else {
                continue;
            };
            if let Some(mut cmd) = lock.pop_front() {
                cmd.player_id = m.slot as i32;
                net.pending[m.slot] = cmd;
            }
        }
    }
}

fn send_client_ticks(
    mut groups: Query<(Entity, &mut DoomTickStream, &DoomParty)>,
    mut players: Query<(&mut DoomSession, &DoomPartyMember)>,
) {
    for (group_ent, mut net, group) in &mut groups {
        if !group.started {
            continue;
        }

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
                TicCmd::default()
            };
        }

        for (mut session, m) in &mut players {
            if m.group == group_ent {
                session.send_cmd_bundle(bundle);
            }
        }

        net.next_tic += 1;
    }
}
