use doom_protocol::{Frame, ToChild, ToParent};
use doomgeneric::game::DoomGeneric;
use doomgeneric::input::KeyData;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

struct DoomContext {
    input_rx: Receiver<ToChild>,
    stream: TcpStream,
    key_queue: VecDeque<KeyData>,
    active: bool,
}

impl DoomContext {
    pub fn new(stream: TcpStream) -> Self {
        let mut reader = stream.try_clone().expect("Clone failed");
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            loop {
                let mut len_buf = [0u8; 4];
                if reader.read_exact(&mut len_buf).is_ok() {
                    let len = u32::from_le_bytes(len_buf) as usize;
                    let mut data = vec![0u8; len];
                    if reader.read_exact(&mut data).is_ok()
                        && let Ok(msg) = postcard::from_bytes(&data)
                    {
                        let _ = tx.send(msg);
                    }
                }
            }
        });

        Self {
            input_rx: rx,
            stream,
            key_queue: VecDeque::new(),
            active: false,
        }
    }

    fn pump_messages(&mut self) {
        while let Ok(msg) = self.input_rx.try_recv() {
            match msg {
                ToChild::Input { input, pressed } => {
                    self.key_queue.push_back(KeyData {
                        pressed,
                        key: input.to_keycode(),
                    });
                }
                ToChild::State { active } => self.active = active,
            }
        }
    }

    fn wait_until_resumed(&mut self) {
        while !self.active {
            match self.input_rx.recv() {
                Ok(ToChild::State { active }) => self.active = active,
                Ok(ToChild::Input {
                    input: _,
                    pressed: _,
                })
                | Err(_) => (),
            }
        }
    }

    fn send_frame(&mut self, frame: &Frame) {
        let msg = ToParent::Frame(*frame);

        if let Ok(encoded) = postcard::to_allocvec(&msg) {
            let res = (|| -> io::Result<()> {
                self.stream
                    .write_all(&(encoded.len() as u32).to_le_bytes())?;
                self.stream.write_all(&encoded)?;
                self.stream.flush()?;
                Ok(())
            })();

            if let Err(e) = res {
                eprintln!("Failed to send frame over TCP: {e}");
            }
        }
    }
}

impl DoomGeneric for DoomContext {
    fn draw_frame(&mut self, screen_buffer: &[u8], xres: usize, yres: usize) {
        self.pump_messages();

        if !self.active {
            self.wait_until_resumed();
        }

        let frame = Frame::from_framebuffer(screen_buffer, xres, yres);
        self.send_frame(&frame);
    }

    fn get_key(&mut self) -> Option<doomgeneric::input::KeyData> {
        self.key_queue.pop_front()
    }

    fn set_window_title(&mut self, _title: &str) {
        // No-op
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let addr = args.get(1).expect("No port provided");

    let stream = TcpStream::connect(addr).expect("Failed to connect to parent");

    let context = DoomContext::new(stream);
    doomgeneric::game::init(context);
    loop {
        doomgeneric::game::tick();
    }
}
