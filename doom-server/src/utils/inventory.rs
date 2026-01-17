use valence::{prelude::Inventory, ItemKind, ItemStack};

#[derive(Debug)]
pub enum SwapError {
    NotFound,
}

pub trait InventoryExt {
    fn clear(&mut self);
    fn fill(&mut self, stack: ItemStack);
    fn swap_first<F>(&mut self, find: F, new_item: ItemStack) -> Result<ItemStack, SwapError>
    where
        F: Fn(&ItemStack) -> bool;
}

impl InventoryExt for Inventory {
    fn fill(&mut self, stack: ItemStack) {
        for i in 0..self.slot_count() {
            self.set_slot(i, stack.clone());
        }
    }

    fn clear(&mut self) {
        self.fill(ItemStack::new(ItemKind::Air, 0, None));
    }

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
}
