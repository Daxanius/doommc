use std::collections::{HashSet, VecDeque};
use valence::prelude::*;

/// Plugin that manages a queue system for players waiting to join DOOM sessions.
/// Players are placed in a queue when all DOOM sessions are full and are
/// admitted to a session when space becomes available.
pub struct QueuePlugin {
    pub capacity: usize,
}

impl Plugin for QueuePlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(QueueCapacity{ available_slots: self.capacity })
        .insert_resource(PlayerQueue::default())
        .add_event::<EnqueuePlayer>()
        .add_event::<DequeuePlayer>()
        .add_event::<PlayerAdmitted>()
        .add_systems(Update, admit_system);
    }
}

#[derive(Resource, Default)]
pub struct PlayerQueue {
    order: VecDeque<Entity>,
    set: HashSet<Entity>, // fast contains/remove guards
}

#[derive(Component)]
pub struct InQueue {
    pub since_tick: i64,
}

/// “Policy input” from the host project.
/// Host updates this each tick (or when capacity changes).
#[derive(Resource, Default)]
pub struct QueueCapacity {
    pub available_slots: usize,
}

/// Host sends this to put a player in the queue.
#[derive(Event)]
pub struct EnqueuePlayer {
    pub player: Entity,
}

/// Host sends this to remove a player from the queue (disconnect, cancel, etc.)
#[derive(Event)]
pub struct DequeuePlayer {
    pub player: Entity,
}

/// Queue plugin emits this when a player is admitted.
#[derive(Event)]
pub struct PlayerAdmitted {
    pub player: Entity,
}

fn admit_system(
    mut q: ResMut<PlayerQueue>,
    mut cap: ResMut<QueueCapacity>,
    mut ev_admitted: EventWriter<PlayerAdmitted>,
    in_queue: Query<(), With<InQueue>>,
) {
    while cap.available_slots > 0 {
        let Some(player) = q.order.pop_front() else {
            break;
        };
        q.set.remove(&player);

        // player might have left / no longer queued
        if in_queue.get(player).is_err() {
            continue;
        }

        cap.available_slots -= 1;
        ev_admitted.send(PlayerAdmitted { player });
    }
}
