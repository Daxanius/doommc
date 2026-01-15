use doom_server::{DoomSession, DoomSessionAllocator};
use valence::{
    inventory::UpdateSelectedSlotEvent,
    prelude::*,
    protocol::{
        packets::play::{map_update_s2c::Data, MapUpdateS2c},
        VarInt, WritePacket,
    },
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (despawn_disconnected_clients,))
        .add_systems(Update, init_clients)
        .add_systems(Update, on_selected_slot_changed)
        .add_systems(Update, tick_all_sessions)
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
    layer
        .chunk
        .set_block([0, 64, 0], BlockState::WHITE_CONCRETE);

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
    ) in &mut clients
    {
        let layer = layers.single();

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.5, 65.0, 0.5]);

        let (doom_session, map) = doom_session_allocator.create_session();
        commands.entity(player).insert(doom_session);
        inventory.set_slot(40, Some(map));
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
