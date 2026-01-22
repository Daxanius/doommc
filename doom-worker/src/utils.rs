use doom_protocol::packet::TicCmd;
use doomgeneric::client::{DoomInputPacketRaw, MAX_PLAYERS};

pub fn packet_to_ticcmd(packet: &DoomInputPacketRaw, maketic: i32, player_id: i32) -> TicCmd {
    TicCmd {
        maketic,
        player_id,
        forwardmove: packet.forwardmove,
        sidemove: packet.sidemove,
        angleturn: packet.angleturn,
        chatchar: packet.chatchar,
        buttons: packet.buttons,
        consistancy: packet.consistancy,
        buttons2: packet.buttons2,
        inventory: packet.inventory,
        lookfly: packet.lookfly,
        arti: packet.arti,
    }
}

pub fn tccmd_to_packet(packet: &TicCmd) -> DoomInputPacketRaw {
    DoomInputPacketRaw {
        forwardmove: packet.forwardmove,
        sidemove: packet.sidemove,
        angleturn: packet.angleturn,
        chatchar: packet.chatchar,
        buttons: packet.buttons,
        consistancy: packet.consistancy,
        buttons2: packet.buttons2,
        inventory: packet.inventory,
        lookfly: packet.lookfly,
        arti: packet.arti,
    }
}

// Returns the raw input packets and tickccmds
pub fn ticcmds_to_bundle(
    packet: &Vec<TicCmd>,
) -> ([DoomInputPacketRaw; MAX_PLAYERS], [i32; MAX_PLAYERS]) {
    let mut inputs = [DoomInputPacketRaw::default(); MAX_PLAYERS];
    let mut player_mask = [0; MAX_PLAYERS];

    for cmd in packet {
        player_mask[cmd.player_id as usize] = 1;
        inputs[cmd.player_id as usize] = tccmd_to_packet(&cmd);
    }

    (inputs, player_mask)
}
