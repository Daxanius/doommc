use serde::{Deserialize, Serialize};

use crate::palette::PALETTE_MAPPING;

pub mod palette;

pub const MAP_WIDTH: usize = 128;
pub const MAP_HEIGHT: usize = 128;

#[derive(Serialize, Deserialize, Debug)]
pub enum ToChild {
    Input { input: Input, pressed: bool },
    State { active: bool },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ToParent {
    Frame(Frame),
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

impl Frame {
    #[must_use]
    pub fn from_framebuffer(buffer: &[u8], src_w: usize, src_h: usize) -> Self {
        // Initialize with a default color index (e.g., 0)
        let mut new_data = [0u8; MAP_WIDTH * MAP_HEIGHT];

        // Calculate mapped height to maintain aspect ratio
        let draw_h = (MAP_WIDTH * src_h) / src_w;
        let v_offset = (MAP_HEIGHT - draw_h) / 2;

        for y in 0..draw_h {
            for x in 0..MAP_WIDTH {
                // Map current y back to the original height
                let src_x = (x * src_w) / MAP_WIDTH;
                let src_y = (y * src_h) / draw_h;

                let src_idx = src_y * src_w + src_x;
                let original_color = buffer[src_idx];

                // Calculate destination index with the vertical offset
                let dest_idx = (y + v_offset) * MAP_WIDTH + x;

                new_data[dest_idx] = PALETTE_MAPPING[original_color as usize];
            }
        }

        Frame(new_data)
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

impl Input {
    #[must_use]
    pub fn to_keycode(&self) -> i32 {
        match self {
            Input::Up => 0xad,
            Input::Down => 0xaf,
            Input::Left => 0xac,
            Input::Right => 0xae,
            Input::Shoot => 0xa3,
            Input::Use => 0xa2,
            Input::Enter => 13,
            Input::StrafeLeft => 0xa0,
            Input::StrafeRight => 0xa1,
        }
    }
}
