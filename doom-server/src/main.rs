use std::{net::SocketAddr, num::NonZero};

use doom_server::{
    plugins::{
        chat::ChatPlugin,
        doom::{
            session::{DoomSession, DoomSessionDirectory, DoomSessionRegistry},
            DoomPluginBundle,
        },
        hotbar::HotbarPlugin,
        queue::{EnqueuePlayer, PlayerAdmitted, QueuePlugin},
    },
    SERVER_RESOURCE_PACK_SHA1_HEX, SERVER_RESOURCE_PACK_URL, TICK_RATE,
};
use valence::{
    command::scopes::CommandScopes,
    message::SendMessage,
    network::{BroadcastToLan, CleanupFn, HandshakeData, ServerListPing},
    prelude::*,
    ServerSettings, MINECRAFT_VERSION,
};

fn main() {
    App::new()
        .insert_resource(NetworkSettings {
            callbacks: CallBacks.into(),
            ..Default::default()
        })
        .insert_resource(ServerSettings {
            tick_rate: NonZero::new(TICK_RATE).unwrap(), // DOOM runs at 35 FPS
            ..Default::default()
        })
        .add_plugins((
            DefaultPlugins,
            DoomPluginBundle,
            ChatPlugin,
            HotbarPlugin,
            QueuePlugin { capacity: 10 },
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, init_clients)
        .add_systems(Update, despawn_disconnected_clients)
        .add_systems(Update, on_player_admitted_start_session)
        .add_systems(Update, on_player_enqueued_give_spectator_map)
        .run();
}

// Create the world
fn setup(
    mut commands: Commands,
    server: Res<Server>,
    biomes: Res<BiomeRegistry>,
    dimensions: Res<DimensionTypeRegistry>,
) {
    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    // We have to add chunks to the world first, they start empty.
    for z in -16..16 {
        for x in -16..16 {
            layer.chunk.insert_chunk([x, z], UnloadedChunk::new());
        }
    }

    // This actually sets the block in the world.
    for z in -1000..1000 {
        for x in -1000..1000 {
            layer.chunk.set_block([x, 64, z], BlockState::RED_CONCRETE);
        }
    }

    // This spawns the layer into the world.
    commands.spawn(layer);
}

// When a player connects, create a doom session for them
#[allow(clippy::type_complexity)]
fn init_clients(
    mut commands: Commands,
    mut d_registry: ResMut<DoomSessionRegistry>,
    mut d_directory: ResMut<DoomSessionDirectory>,
    mut clients: Query<
        (
            Entity,
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut CommandScopes,
            &mut Position,
            &mut GameMode,
            &mut Inventory,
            &mut Client,
        ),
        Added<Client>,
    >,
    mut sessions: Query<&mut DoomSession>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    for (
        entity,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut permissions,
        mut pos,
        mut game_mode,
        mut inventory,
        mut client,
    ) in &mut clients
    {
        let layer = layers.single();
        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.5, 65.0, 0.5]);
        *game_mode = GameMode::Creative;
        permissions.add("doom.admin");

        let (mut session, map) = d_registry.create_session("doom.wad");
        session.add_player(session.id().abs() - 1);

        // Add player to other peoples' multiplayer sessions for now
        for mut s in &mut sessions {
            s.add_player(session.id().abs() - 1);
            println!("Adding player {} for session: {}", session.id(), s.id());
        }

        d_directory.insert(entity, session.id());
        commands.entity(entity).insert(session);

        inventory.set_slot(40, map);

        client.set_resource_pack(
            SERVER_RESOURCE_PACK_URL,
            SERVER_RESOURCE_PACK_SHA1_HEX,
            false,
            Some("Optional but provides a more integral experience".into_text()),
        );
    }
}

fn on_player_enqueued_give_spectator_map(
    mut doom_registry: ResMut<DoomSessionRegistry>,
    mut ev: EventReader<EnqueuePlayer>,
    mut inventories: Query<&mut Inventory>,
    mut clients: Query<&mut Client>,
) {
    for EnqueuePlayer { player } in ev.read() {
        let Ok(mut inv) = inventories.get_mut(*player) else {
            continue;
        };
        let Ok(mut client) = clients.get_mut(*player) else {
            continue;
        };

        if let Some(map) = doom_registry.create_random_map_view() {
            inv.set_slot(40, map);
            client.send_action_bar_message("You're in the DOOM queue. Watching an active session…");
        } else {
            client.send_action_bar_message(
                "You're in the DOOM queue. No active sessions to watch yet.",
            );
        }
    }
}

fn on_player_admitted_start_session(
    mut commands: Commands,
    mut doom_registry: ResMut<DoomSessionRegistry>,
    mut ev: EventReader<PlayerAdmitted>,
    mut inventories: Query<&mut Inventory>,
    mut clients: Query<&mut Client>,
) {
    for PlayerAdmitted { player } in ev.read() {
        let Ok(mut inv) = inventories.get_mut(*player) else {
            continue;
        };
        let Ok(mut client) = clients.get_mut(*player) else {
            continue;
        };

        let (session, map) = doom_registry.create_session("doom.wad");
        commands.entity(*player).insert(session);
        inv.set_slot(40, map);

        client.set_title("You're up! Your DOOM session has started.");
    }
}

struct CallBacks;

#[async_trait::async_trait]
impl NetworkCallbacks for CallBacks {
    async fn server_list_ping(
        &self,
        _shared: &SharedNetworkState,
        _remote_addr: SocketAddr,
        handshake_data: &HandshakeData,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: 1,
            max_players: 1,
            player_sample: vec![],
            description: "Get ready to RIP AND TEAR".into_text(),
            favicon_png: include_bytes!("../assets/logo.png"),
            version_name: ("Valence ".color(Color::GOLD) + MINECRAFT_VERSION.color(Color::RED))
                .to_legacy_lossy(),
            protocol: handshake_data.protocol_version,
        }
    }

    async fn broadcast_to_lan(&self, _shared: &SharedNetworkState) -> BroadcastToLan {
        BroadcastToLan::Enabled("Get ready to RIP AND TEAR!".into())
    }

    async fn login(
        &self,
        _shared: &SharedNetworkState,
        _info: &NewClientInfo,
    ) -> Result<CleanupFn, Text> {
        Ok(Box::new(move || {}))
    }
}
