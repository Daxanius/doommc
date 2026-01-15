use crate::palette::PALETTE_MAPPING;

pub mod palette;

pub const MAP_WIDTH: usize = 128;
pub const MAP_HEIGHT: usize = 128;

pub struct Frame([u8; MAP_WIDTH * MAP_HEIGHT]);

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

enum Input {
    Up,
    Down,
    Left,
    Right,
    Shoot,
    Use,
}

impl Input {
    pub fn to_keycode(&self) -> i32 {
        match self {
            Input::Up => 200,
            Input::Down => 208,
            Input::Left => 203,
            Input::Right => 205,
            Input::Shoot => 57, // space
            Input::Use => 17,   // W
        }
    }
}
