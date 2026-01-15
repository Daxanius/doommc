pub mod palette;

struct Frame([u8; 128 * 128]);

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
