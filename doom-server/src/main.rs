use doom_server::DoomSession;
use valence::{
    inventory::{self, InventoryPlugin, UpdateSelectedSlotEvent},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (despawn_disconnected_clients,))
        .add_systems(Update, init_clients)
        .add_systems(Update, on_selected_slot_changed)
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
        player,
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

        use valence::nbt::Compound;

        let mut tag = Compound::new();
        tag.insert("map", uuid::Uuid::new_v4()); // map id
        let doom_map = ItemStack::new(ItemKind::FilledMap, 1, Some(tag));

        inventory.set_slot(40, Some(doom_map));
    }
}

fn on_selected_slot_changed(
    mut ev: EventReader<UpdateSelectedSlotEvent>,
    mut q: Query<(&Inventory, &mut DoomSession)>,
) {
    for e in &mut ev {
        let (inv, mut session) = q.get_mut(e.client).unwrap();

        // e.slot is 0..8 (hotbar index)
        let slot_id = 36 + u16::from(e.slot);
        let stack = inv.slot(slot_id); // whatever accessor you use

        session.active = matches!(stack, Some(s) if is_doom_map(s, session.map_id));
    }
}

fn is_doom_map(stack: &ItemStack, _map_id: Uuid) -> bool {
    if stack.item != ItemKind::FilledMap {
        return false;
    }

    true
}
