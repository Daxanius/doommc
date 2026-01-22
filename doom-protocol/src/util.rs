use crate::{
    MAP_HEIGHT, MAP_WIDTH,
    packet::{Frame, Input},
    palette::PALETTE_MAPPING,
};

impl Frame {
    #[must_use]
    pub fn from_framebuffer(buffer: &[u8], src_w: usize, src_h: usize) -> Self {
        let mut out = [0u8; MAP_WIDTH * MAP_HEIGHT];

        let dst_w = MAP_WIDTH as f32;
        let dst_h = MAP_HEIGHT as f32;
        let src_wf = src_w as f32;
        let src_hf = src_h as f32;

        // "Cover" scale: fill the entire destination, cropping overflow.
        let scale = (dst_w / src_wf).max(dst_h / src_hf);

        let scaled_w = src_wf * scale;
        let scaled_h = src_hf * scale;

        // Center crop in scaled space
        let crop_x = (scaled_w - dst_w) * 0.5;
        let crop_y = (scaled_h - dst_h) * 0.5;

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                // Destination pixel -> scaled source space (+ crop), then -> original source space
                let sx = ((x as f32 + crop_x) / scale).round() as isize;
                let sy = ((y as f32 + crop_y) / scale).round() as isize;

                // Clamp to be safe
                let sx = sx.clamp(0, (src_w as isize) - 1) as usize;
                let sy = sy.clamp(0, (src_h as isize) - 1) as usize;

                let src_idx = sy * src_w + sx;
                let original = buffer[src_idx];

                out[y * MAP_WIDTH + x] = PALETTE_MAPPING[original as usize];
            }
        }

        Frame(out)
    }
}

impl Input {
    #[must_use]
    pub fn to_keycode(&self) -> u8 {
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

#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn float_to_delta(delta: f32) -> i16 {
    let sensitivity = -128.0;
    let scaled_delta = (delta * sensitivity) as i32;
    scaled_delta.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}
