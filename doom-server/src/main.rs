use std::net::SocketAddr;

use doom_server::plugins::{
    chat::ChatPlugin,
    doom::{DoomPlugin, DoomSessionRegistry},
    queue::{EnqueuePlayer, PlayerAdmitted, QueuePlugin},
};
use valence::{
    message::SendMessage,
    network::{BroadcastToLan, CleanupFn, HandshakeData, ServerListPing},
    prelude::*,
    MINECRAFT_VERSION,
};

fn main() {
    App::new()
        .insert_resource(NetworkSettings {
            callbacks: CallBacks.into(),
            ..Default::default()
        })
        .add_plugins((
            DefaultPlugins,
            DoomPlugin,
            ChatPlugin,
            QueuePlugin { capacity: 10 },
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, init_clients)
        .add_systems(Update, on_player_admitted_start_session)
        .add_systems(Update, on_player_enqueued_give_spectator_map)
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
    mut d_registry: ResMut<DoomSessionRegistry>,
    mut clients: Query<
        (
            Entity,
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
            &mut GameMode,
            &mut Inventory,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    for (
        entity,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut pos,
        mut game_mode,
        mut inventory,
    ) in &mut clients
    {
        let layer = layers.single();
        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.5, 65.0, 0.5]);
        *game_mode = GameMode::Creative;

        let (session, map) = d_registry.create_session();
        commands.entity(entity).insert(session);

        inventory.set_slot(40, map);
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

        if let Some(map) = doom_registry.create_random_view_map() {
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

        let (session, map) = doom_registry.create_session();
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
            favicon_png: include_bytes!("../assets/logo-64x64.png"),
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
