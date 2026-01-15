use doomgeneric::game::DoomGeneric;

struct DoomHandler {}

impl DoomHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl DoomGeneric for DoomHandler {
    fn draw_frame(&mut self, screen_buffer: &[u8], xres: usize, yres: usize) {
        todo!()
    }

    fn get_key(&mut self) -> Option<doomgeneric::input::KeyData> {
        todo!()
    }

    fn set_window_title(&mut self, title: &str) {
        todo!()
    }
}

fn main() {
    println!("Hello, world!");
}
