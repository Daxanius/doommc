use valence::prelude::*;

use crate::extensions::inventory::InventoryExt;
use crate::extensions::item::ItemStackExt;
use crate::extensions::nbt::NbtValue;

#[must_use]
#[inline]
pub fn wrap_degrees(mut d: f32) -> f32 {
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

#[must_use]
#[inline]
pub fn should_be_active(session_id: i32, inventory: &Inventory, selected: u8) -> bool {
    let stack = inventory.hotbar_slot(selected);
    stack.is_kind(ItemKind::FilledMap) && stack.has_tag_with("map", &NbtValue::from(session_id).0)
}
