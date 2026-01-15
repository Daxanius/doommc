use doom_protocol::Frame;
use doomgeneric::game::DoomGeneric;

struct DoomHandler;

impl DoomHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl DoomGeneric for DoomHandler {
    fn draw_frame(&mut self, screen_buffer: &[u8], xres: usize, yres: usize) {
        let frame = Frame::from_framebuffer(screen_buffer, xres, yres);
        println!("Received frame!");
    }

    fn get_key(&mut self) -> Option<doomgeneric::input::KeyData> {
        None
    }

    fn set_window_title(&mut self, _title: &str) {
        // No-op
    }
}

fn main() {
    let handler = DoomHandler::new();
    doomgeneric::game::init(handler);

    loop {
        doomgeneric::game::tick();
    }
}
