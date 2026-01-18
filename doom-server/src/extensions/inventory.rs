use valence::{prelude::Inventory, ItemKind, ItemStack};

use crate::extensions::item::ItemStackExt;

pub const HOTBAR_SIZE: u16 = 9;

#[derive(Debug)]
pub enum SwapError {
    NotFound,
}

pub trait InventoryExt {
    fn clear(&mut self);
    fn fill(&mut self, stack: ItemStack);

    /// Swaps the first item stack matching the predicate with a new item.
    ///
    /// # Errors
    ///
    /// Returns `SwapError::NotFound` if no item stack matches the predicate.
    fn swap_first<F>(&mut self, find: F, new_item: ItemStack) -> Result<ItemStack, SwapError>
    where
        F: Fn(&ItemStack) -> bool;

    fn replace_first(&mut self, new_item: ItemStack) -> bool;

    fn find_stacks_of(&self, kind: ItemKind) -> Vec<&ItemStack>;

    fn hotbar_slot(&self, idx: u8) -> &ItemStack;
}

impl InventoryExt for Inventory {
    #[inline]
    fn fill(&mut self, stack: ItemStack) {
        for i in 0..self.slot_count() {
            self.set_slot(i, stack.clone());
        }
    }

    #[inline]
    fn clear(&mut self) {
        self.fill(ItemStack::new(ItemKind::Air, 0, None));
    }

    #[inline]
    fn swap_first<F>(&mut self, find: F, new_item: ItemStack) -> Result<ItemStack, SwapError>
    where
        F: Fn(&ItemStack) -> bool,
    {
        for idx in 0..self.slot_count() {
            let slot = self.slot(idx).clone();

            if find(&slot) {
                self.set_slot(idx, new_item.clone());
                return Ok(slot);
            }
        }

        Err(SwapError::NotFound)
    }

    #[inline]
    fn replace_first(&mut self, new_item: ItemStack) -> bool {
        self.swap_first(|stack| stack.is_kind(new_item.item), new_item.clone())
            .is_ok()
    }

    #[inline]
    fn find_stacks_of(&self, kind: ItemKind) -> Vec<&ItemStack> {
        let mut stacks = Vec::<&ItemStack>::new();

        for idx in 0..self.slot_count() {
            let stack = self.slot(idx);
            if stack.item == kind {
                stacks.push(stack);
            }
        }

        stacks
    }

    #[inline]
    fn hotbar_slot(&self, idx: u8) -> &ItemStack {
        self.slot(u16::from(idx) + (self.slot_count() - HOTBAR_SIZE - 1))
    }
}
