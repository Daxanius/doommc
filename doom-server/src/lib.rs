use valence::prelude::*;

#[derive(Component)]
pub struct DoomSession {
    pub map_id: Uuid,
    pub active: bool,
}
