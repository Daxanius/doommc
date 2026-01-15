use valence::nbt::Compound;
use valence::prelude::*;

#[derive(Component)]
pub struct DoomSession {
    id: i32,
    pub active: bool,
}

impl DoomSession {
    #[must_use]
    fn from_id(id: i32) -> Self {
        Self { id, active: true }
    }

    #[must_use]
    pub fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Resource, Default)]
pub struct DoomSessionAllocator {
    next_id: i32,
}

impl DoomSessionAllocator {
    #[must_use]
    pub fn create_session(&mut self) -> (DoomSession, ItemStack) {
        let id = self.allocate();
        let mut tag = Compound::new();
        tag.insert("map", id);
        let map = ItemStack::new(ItemKind::FilledMap, 1, Some(tag));
        (DoomSession::from_id(id), map)
    }

    fn allocate(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
