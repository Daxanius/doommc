use valence::{
    app::{App, Plugin},
    inventory::UpdateSelectedSlotEvent,
    prelude::*,
};

use crate::extensions::inventory::InventoryExt;

/// Plugin that adds chat functionality to the server.
/// Players can send and receive messages in the chat.
pub struct HotbarPlugin;

impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .add_systems(Update, init_clients)
        .add_systems(Update, sync_held_item_from_hotbar);
    }
}

#[derive(Component, Default)]
pub struct SelectedHotbarSlot(pub u8);

#[allow(clippy::type_complexity)]
fn init_clients(mut commands: Commands, mut clients: Query<Entity, Added<Client>>) {
    for entity in &mut clients {
        commands
            .entity(entity)
            .insert(SelectedHotbarSlot::default());
    }
}

fn sync_held_item_from_hotbar(
    mut ev: EventReader<UpdateSelectedSlotEvent>,
    mut q: Query<(&Inventory, &mut SelectedHotbarSlot, &mut Equipment), With<Client>>,
) {
    for e in ev.read() {
        let Ok((inv, mut selected, mut equip)) = q.get_mut(e.client) else {
            continue;
        };

        selected.0 = e.slot;

        let held = inv.hotbar_slot(e.slot);
        equip.set_main_hand(held.clone());
    }
}
