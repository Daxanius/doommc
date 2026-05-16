use serde::{Deserialize, Serialize};

use crate::{MAP_HEIGHT, MAP_WIDTH, MAX_PLAYERS};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct TicCmd {
    pub maketic: i32,   // The tick this command was generated at
    pub player_id: i32, // The player that generated this command

    pub forwardmove: i8, // signed char
    pub sidemove: i8,    // signed char
    pub angleturn: i16,  // short
    pub chatchar: i8,    // byte
    pub buttons: i8,     // byte
    pub consistancy: i8, // byte

    // Strife specific
    pub buttons2: i8,   // byte
    pub inventory: i32, // int

    // Heretic/Hexen specific
    pub lookfly: i8, // byte
    pub arti: i8,    // byte
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct DoomGameSettings {
    pub ticdup: i32,
    pub extratics: i32,
    pub deathmatch: i32,
    pub episode: i32,
    pub nomonsters: i32,
    pub fast_monsters: i32,
    pub respawn_monsters: i32,
    pub map: i32,
    pub skill: i32,
    pub gameversion: i32,
    pub lowres_turn: i32,
    pub new_sync: i32,
    pub timelimit: i32,
    pub loadgame: i32,
    pub random: i32,

    // Start message fields
    pub num_players: i32,
    pub consoleplayer: i32,

    // Hexen classes (ensure NET_MAXPLAYERS matches C, usually 4 or 8)
    pub player_classes: [i32; 8],
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct CmdBundle {
    pub maketic: i32,
    pub present: [bool; MAX_PLAYERS],
    pub cmds: [TicCmd; MAX_PLAYERS],
}

#[derive(Debug, Clone, Copy)]
pub struct Frame(pub [u8; MAP_WIDTH * MAP_HEIGHT]);

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec: Vec<u8> = Deserialize::deserialize(deserializer)?;
        let mut array = [0u8; MAP_WIDTH * MAP_HEIGHT];
        if vec.len() != MAP_WIDTH * MAP_HEIGHT {
            return Err(serde::de::Error::custom("invalid frame data length"));
        }
        array.copy_from_slice(&vec);
        Ok(Frame(array))
    }
}

impl Serialize for Frame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_vec().serialize(serializer)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Shoot,
    Use,
    Enter,
    StrafeLeft,
    StrafeRight,
}
