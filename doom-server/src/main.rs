use std::net::SocketAddr;

use doom_protocol::Input;
use doom_server::{DoomSession, DoomSessionAllocator};
use valence::{
    hand_swing::HandSwingEvent,
    interact_item::InteractItemEvent,
    inventory::UpdateSelectedSlotEvent,
    math::Vec3Swizzles,
    message::ChatMessageEvent,
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
        .add_systems(Update, on_selected_slot_changed)
        .add_systems(Update, on_player_move)
        .add_systems(Update, tick_all_sessions)
        .add_systems(Update, input_on_message)
        .add_systems(Update, detect_player_stop)
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
        commands
            .entity(player)
            .insert(doom_session)
            .insert(MovementTracker { last_tick: 0 });
        inventory.set_slot(40, Some(map));
        *game_mode = GameMode::Creative;
    }
}

fn on_selected_slot_changed(
    mut ev: EventReader<UpdateSelectedSlotEvent>,
    mut q: Query<(&Inventory, &mut DoomSession)>,
) {
    for e in &mut ev {
        for (inv, mut session) in &mut q {
            println!("Player selected slot {:?}", e.slot);
            let slot_id = u16::from(e.slot);
            let stack = inv.slot(slot_id); // whatever accessor you use

            session.active = matches!(stack, Some(s) if is_doom_map(s));
        }
    }
}

#[derive(Component)]
pub struct MovementTracker {
    pub last_tick: i64,
}

fn detect_player_stop(server: Res<Server>, mut q: Query<(&mut DoomSession, &MovementTracker)>) {
    let current_tick = server.current_tick();

    for (mut session, tracker) in &mut q {
        if current_tick > tracker.last_tick + 20 {
            session.set_input(Input::Up, false);
            session.set_input(Input::Down, false);
            session.set_input(Input::Left, false);
            session.set_input(Input::Right, false);
        }
    }
}

fn on_player_move(
    mut ev: EventReader<MovementEvent>,
    server: Res<Server>,
    mut q: Query<(&mut Position, &mut DoomSession, &mut MovementTracker)>,
) {
    for e in &mut ev {
        for (mut pos, mut session, mut tracker) in &mut q {
            let delta = e.position - e.old_position;

            // Calculate local direction vectors from Yaw
            let yaw_rad = e.look.yaw.to_radians();
            let forward_v = Vec2::new(-yaw_rad.sin(), -yaw_rad.cos()).normalize();
            let right_v = Vec2::new(-yaw_rad.cos(), yaw_rad.sin()).normalize();

            // Project movement onto our local vectors
            let forward_dot = delta.xz().dot(forward_v.as_dvec2());
            let right_dot = delta.xz().dot(right_v.as_dvec2());

            // Thresholds to trigger the Enum inputs
            let threshold = 0.05;

            // Send Forward/Backward
            session.set_input(Input::Up, forward_dot < -threshold);
            session.set_input(Input::Down, forward_dot > threshold);

            session.set_input(Input::Right, right_dot > threshold);
            session.set_input(Input::Left, right_dot < -threshold);

            pos.set(e.old_position);
            tracker.last_tick = server.current_tick();
        }
    }
}

fn on_player_sneak(mut ev: EventReader<SneakEvent>, mut q: Query<&mut DoomSession>) {
    for e in &mut ev {
        for mut session in &mut q {
            session.set_input(Input::Shoot, e.state == SneakState::Start);
            session.set_input(Input::Enter, e.state == SneakState::Start);
        }
    }
}

fn on_player_interact(mut ev: EventReader<HandSwingEvent>, mut q: Query<&mut DoomSession>) {
    for _e in &mut ev {
        for mut session in &mut q {
            session.toggle_input(Input::Use);
        }
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

fn is_doom_map(stack: &ItemStack) -> bool {
    stack.item == ItemKind::FilledMap
}

fn input_on_message(mut events: EventReader<ChatMessageEvent>, mut q: Query<&mut DoomSession>) {
    for event in &mut events {
        for mut session in &mut q {
            let message = event.message.trim();

            match message {
                "w" => session.toggle_input(Input::Up),
                "s" => session.toggle_input(Input::Down),
                "a" => session.toggle_input(Input::Left),
                "d" => session.toggle_input(Input::Right),
                "x" => session.toggle_input(Input::Shoot),
                "e" => session.toggle_input(Input::Use),
                "f" => session.toggle_input(Input::Enter),
                _ => (),
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
