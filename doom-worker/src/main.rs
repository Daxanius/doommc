use doom_protocol::{Frame, ToChild, ToParent};
use doomgeneric::game::DoomGeneric;
use doomgeneric::input::KeyData;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

struct DoomHandler {
    input_rx: Receiver<ToChild>,
    stream: TcpStream,
    key_queue: VecDeque<KeyData>,
}

impl DoomHandler {
    pub fn new(input_rx: Receiver<ToChild>, stream: TcpStream) -> Self {
        Self {
            input_rx,
            stream,
            key_queue: VecDeque::new(),
        }
    }
}

impl DoomGeneric for DoomHandler {
    fn draw_frame(&mut self, screen_buffer: &[u8], xres: usize, yres: usize) {
        let frame = Frame::from_framebuffer(screen_buffer, xres, yres);
        let msg = ToParent::Frame(frame);

        // Serialize the message
        if let Ok(encoded) = postcard::to_allocvec(&msg) {
            // Write to the TCP Stream instead of stdout
            // We use a block or a reference to ensure we don't move the stream
            let res = (|| -> io::Result<()> {
                // 1. Write the 4-byte length prefix
                self.stream
                    .write_all(&(encoded.len() as u32).to_le_bytes())?;
                // 2. Write the actual data
                self.stream.write_all(&encoded)?;
                // 3. Flush to ensure the parent receives it immediately
                self.stream.flush()?;
                Ok(())
            })();

            if let Err(e) = res {
                eprintln!("Failed to send frame over TCP: {e}");
            }
        }
    }

    fn get_key(&mut self) -> Option<doomgeneric::input::KeyData> {
        // Pull everything from the channel and put it in our local queue
        while let Ok(ToChild::Input { input, pressed }) = self.input_rx.try_recv() {
            self.key_queue.push_back(KeyData {
                pressed,
                key: input.to_keycode(),
            });
        }

        // Return the next event in the queue to Doom
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
    let mut reader = stream.try_clone().expect("Clone failed");
    let (tx, rx) = mpsc::channel();

    // Input Thread: Reads from Parent
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

    let handler = DoomHandler::new(rx, stream);
    doomgeneric::game::init(handler);

    loop {
        doomgeneric::game::tick();
    }
}
