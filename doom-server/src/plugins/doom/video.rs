use valence::prelude::*;
use valence::protocol::packets::play::map_update_s2c::Data;
use valence::protocol::packets::play::MapUpdateS2c;
use valence::protocol::{VarInt, WritePacket};

use crate::plugins::doom::session::DoomSession;
use crate::plugins::doom::util::should_be_active;
use crate::plugins::hotbar::SelectedHotbarSlot;

/// Plugin that adds doom session video streaming for Minecraft maps
/// each map ID corresponds to a session ID. Supports viewing other peoples' sessions too.
pub struct DoomVideoPlugin;

impl Plugin for DoomVideoPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .add_systems(
            Update,
            (
                update_active_sessions,
            ),
        );
    }
}

fn update_active_sessions(
    mut q: Query<&mut DoomSession>,
    mut clients: Query<(&mut Client, &Inventory, &SelectedHotbarSlot)>,
) {
    for session in &mut q {
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

                for (mut client, inventory, selected_slot) in &mut clients {
                    // Check if the client is holding a map with this session ID (aka viewing / spectating this session)
                    if should_be_active(session.id(), inventory, selected_slot.0) {
                        client.write_packet(&pkt);
                    }
                }
            }
        }
    }
}
