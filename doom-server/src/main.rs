use std::net::SocketAddr;

use doom_server::plugins::doom::DoomPlugin;
use valence::{
    network::{BroadcastToLan, CleanupFn, HandshakeData, ServerListPing},
    prelude::*,
    MINECRAFT_VERSION,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DoomPlugin)
        .insert_resource(NetworkSettings {
            callbacks: CallBacks.into(),
            ..Default::default()
        })
        .add_systems(Startup, setup)
        .add_systems(Update, despawn_disconnected_clients)
        .add_systems(Update, init_clients)
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
    mut clients: Query<
        (
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
            &mut GameMode,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    for (
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut pos,
        mut game_mode,
    ) in &mut clients
    {
        let layer = layers.single();

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.5, 65.0, 0.5]);
        *game_mode = GameMode::Creative;
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
