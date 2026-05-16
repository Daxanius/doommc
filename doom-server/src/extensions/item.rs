use valence::{nbt::Value, ItemKind, ItemStack};

use crate::extensions::nbt::nbt_equals;

pub trait ItemStackExt {
    fn has_tag_with(&self, tag: &str, val: &Value) -> bool;

    fn is_kind(&self, kind: ItemKind) -> bool;

    fn compare(&self, other: ItemStack) -> bool;
}

impl ItemStackExt for ItemStack {
    #[inline]
    fn has_tag_with(&self, tag: &str, val: &Value) -> bool {
        self.nbt
            .as_ref()
            .and_then(|data| data.get(tag))
            .is_some_and(|data| nbt_equals(data, val))
    }

    #[inline]
    fn is_kind(&self, kind: ItemKind) -> bool {
        self.item == kind
    }

    #[inline]
    fn compare(&self, other: ItemStack) -> bool {
        self.item == other.item && self.nbt == other.nbt
    }
}
