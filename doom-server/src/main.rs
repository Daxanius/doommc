use std::net::SocketAddr;

use doom_protocol::Input;
use doom_server::{DoomSession, DoomSessionAllocator};
use valence::{
    hand_swing::HandSwingEvent,
    inventory::{self, UpdateSelectedSlotEvent},
    math::Vec3Swizzles,
    movement::MovementEvent,
    network::{BroadcastToLan, CleanupFn, HandshakeData, ServerListPing},
    prelude::*,
    protocol::{
        packets::play::{map_update_s2c::Data, MapUpdateS2c},
        VarInt, WritePacket,
    },
    MINECRAFT_VERSION,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(NetworkSettings {
            callbacks: CallBacks.into(),
            ..Default::default()
        })
        .add_systems(Startup, setup)
        .add_systems(Update, despawn_disconnected_clients)
        .add_systems(Update, on_player_sneak)
        .add_systems(Update, init_clients)
        .add_systems(Update, on_player_interact)
        .add_systems(Update, on_player_move)
        .add_systems(Update, freeze_player)
        .add_systems(Update, tick_all_sessions)
        .add_systems(Update, detect_player_stop)
        .add_systems(Update, on_slot_selected)
        .insert_resource(DoomSessionAllocator::default())
        .run();
}

fn setup(
    mut commands: Commands,
    server: Res<Server>,
    biomes: Res<BiomeRegistry>,
    dimensions: Res<DimensionTypeRegistry>,
) {
    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    // We have to add chunks to the world first, they start empty.
    for z in -5..5 {
        for x in -5..5 {
            layer.chunk.insert_chunk([x, z], UnloadedChunk::new());
        }
    }

    // This actually sets the block in the world.
    for z in -10..10 {
        for x in -10..10 {
            layer.chunk.set_block([x, 64, z], BlockState::RED_CONCRETE);
        }
    }

    // This spawns the layer into the world.
    commands.spawn(layer);
}

#[allow(clippy::type_complexity)]
fn init_clients(
    mut commands: Commands,
    mut doom_session_allocator: ResMut<DoomSessionAllocator>,
    mut clients: Query<
        (
            Entity,
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
            &mut Inventory,
            &mut GameMode,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    for (
        player,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut pos,
        mut inventory,
        mut game_mode,
    ) in &mut clients
    {
        let layer = layers.single();

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.5, 65.0, 0.5]);

        let (doom_session, map) = doom_session_allocator.create_session();
        commands.entity(player).insert(doom_session);
        inventory.set_slot(40, map);
        *game_mode = GameMode::Creative;
    }
}

#[derive(Component, Default)]
pub struct MovementTracker {
    pub last_tick: i64,
    pub last_yaw: f32,
    pub turning_left: bool,
    pub turning_right: bool,
    pub moving_forward: bool,
    pub moving_back: bool,
    pub strafing_left: bool,
    pub strafing_right: bool,
    pub freeze_position: DVec3,
}

impl MovementTracker {
    pub fn new(freeze_position: DVec3) -> Self {
        Self {
            freeze_position,
            ..Default::default()
        }
    }
}

fn freeze_player(mut q: Query<(&mut Position, &MovementTracker)>) {
    for (mut position, tracker) in &mut q {
        position.0 = tracker.freeze_position;
    }
}

fn detect_player_stop(server: Res<Server>, mut q: Query<(&mut DoomSession, &MovementTracker)>) {
    let current_tick = server.current_tick();

    for (mut session, tracker) in &mut q {
        if current_tick > tracker.last_tick + 10 {
            session.set_input(Input::Up, false);
            session.set_input(Input::Down, false);
            session.set_input(Input::Left, false);
            session.set_input(Input::Right, false);
        }
    }
}

fn wrap_degrees(mut d: f32) -> f32 {
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

fn on_player_move(
    mut ev: EventReader<MovementEvent>,
    server: Res<Server>,
    mut q: Query<(&mut DoomSession, &mut MovementTracker)>,
) {
    const TURN_ON: f32 = 3.0;
    const TURN_OFF: f32 = 1.5;
    const MOVE_ON: f64 = 0.03;
    const MOVE_OFF: f64 = 0.015;

    for e in ev.read() {
        let Ok((mut session, mut tr)) = q.get_mut(e.client) else {
            continue;
        };

        // turn from yaw delta
        let dyaw = wrap_degrees(e.look.yaw - tr.last_yaw);

        let turn_right = if tr.turning_right {
            dyaw > TURN_OFF
        } else {
            dyaw > TURN_ON
        };
        let turn_left = if tr.turning_left {
            dyaw < -TURN_OFF
        } else {
            dyaw < -TURN_ON
        };

        tr.turning_right = turn_right;
        tr.turning_left = turn_left;

        session.set_input(Input::Right, turn_right);
        session.set_input(Input::Left, turn_left);

        tr.last_yaw = e.look.yaw;

        // movement intent from delta
        let delta = e.position - e.old_position;

        let yaw = e.look.yaw.to_radians();
        let forward = Vec2::new(-yaw.sin(), yaw.cos()).normalize();
        let right = -Vec2::new(yaw.cos(), yaw.sin()).normalize();

        let f = delta.xz().dot(forward.as_dvec2());
        let r = delta.xz().dot(right.as_dvec2());

        let forward_on = if tr.moving_forward {
            f > MOVE_OFF
        } else {
            f > MOVE_ON
        };
        let back_on = if tr.moving_back {
            f < -MOVE_OFF
        } else {
            f < -MOVE_ON
        };

        tr.moving_forward = forward_on;
        tr.moving_back = back_on;

        session.set_input(Input::Up, forward_on);
        session.set_input(Input::Down, back_on);

        let strafe_r = if tr.strafing_right {
            r > MOVE_OFF
        } else {
            r > MOVE_ON
        };
        let strafe_l = if tr.strafing_left {
            r < -MOVE_OFF
        } else {
            r < -MOVE_ON
        };

        tr.strafing_right = strafe_r;
        tr.strafing_left = strafe_l;

        session.set_input(Input::StrafeRight, strafe_r);
        session.set_input(Input::StrafeLeft, strafe_l);

        tr.last_tick = server.current_tick();
    }
}

fn on_player_sneak(mut ev: EventReader<SneakEvent>, mut q: Query<&mut DoomSession>) {
    for e in &mut ev.read() {
        let Ok(mut session) = q.get_mut(e.client) else {
            continue;
        };

        let down = e.state == SneakState::Start;
        session.set_input(Input::Shoot, down);
        session.set_input(Input::Enter, down);
    }
}

fn on_slot_selected(
    mut commands: Commands,
    mut ev: EventReader<UpdateSelectedSlotEvent>,
    mut q: Query<(Entity, &mut DoomSession, &Inventory, &Position)>,
) {
    for e in &mut ev.read() {
        let Ok((player, mut session, inventory, position)) = q.get_mut(e.client) else {
            continue;
        };

        let active = inventory.slot((e.slot + 36).into()).item == ItemKind::FilledMap;
        let state_changed = session.set_active(active);

        if state_changed {
            if active {
                commands
                    .entity(player)
                    .insert(MovementTracker::new(position.0));
            } else {
                commands.entity(player).remove::<MovementTracker>();
            }
        }
    }
}

fn on_player_interact(mut ev: EventReader<HandSwingEvent>, mut q: Query<&mut DoomSession>) {
    for e in &mut ev.read() {
        let Ok(mut session) = q.get_mut(e.client) else {
            continue;
        };

        session.toggle_input(Input::Use);
    }
}

fn tick_all_sessions(mut q: Query<(&mut Client, &mut DoomSession)>) {
    for (mut client, session) in &mut q {
        if !session.active {
            continue;
        }

        if let Ok(mut frame_lock) = session.latest_frame.try_lock() {
            if let Some(frame) = frame_lock.take() {
                let pkt = MapUpdateS2c {
                    map_id: VarInt(session.id()),
                    scale: 0,
                    locked: true,
                    icons: None,
                    data: Some(Data {
                        columns: 128,
                        rows: 128,
                        position: [0, 0],
                        data: &frame.0,
                    }),
                };

                client.write_packet(&pkt);
            }
        }
    }
}

struct CallBacks;

#[async_trait::async_trait]
impl NetworkCallbacks for CallBacks {
    async fn server_list_ping(
        &self,
        _shared: &SharedNetworkState,
        remote_addr: SocketAddr,
        handshake_data: &HandshakeData,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: 0,
            max_players: 10,
            player_sample: vec![],
            description: "Get ready to RIP AND TEAR".into_text(),
            favicon_png: include_bytes!("../assets/logo-64x64.png"),
            version_name: ("Valence ".color(Color::GOLD) + MINECRAFT_VERSION.color(Color::RED))
                .to_legacy_lossy(),
            protocol: handshake_data.protocol_version,
        }
    }

    async fn broadcast_to_lan(&self, _shared: &SharedNetworkState) -> BroadcastToLan {
        BroadcastToLan::Enabled("DOOM!".into())
    }

    async fn login(
        &self,
        _shared: &SharedNetworkState,
        info: &NewClientInfo,
    ) -> Result<CleanupFn, Text> {
        let username = info.username.clone();

        Ok(Box::new(move || {
            println!("Cleaning up client: {username}");
        }))
    }
}
