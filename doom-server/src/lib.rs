use doom_protocol::{Frame, ToChild};
use std::env;
use std::net::TcpListener;
use std::process::Child;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use valence::nbt::Compound;
use valence::prelude::*;

#[derive(Component)]
pub struct DoomSession {
    id: i32,
    pub active: bool,
    // The latest frame received from the worker
    pub latest_frame: Arc<Mutex<Option<Frame>>>,
    // Channel to send inputs to the worker
    pub input_tx: Sender<ToChild>,
    // Keep handle to child process so it doesn't drop
    pub child_handle: Child,

    pressed_keys: Vec<doom_protocol::Input>,
}

impl DoomSession {
    #[must_use]
    fn from_id(id: i32) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let exe_path = env::current_exe().expect("Failed to get current exe path");
        let bin_dir = exe_path.parent().expect("Failed to get bin directory");

        let mut worker_path = bin_dir.to_path_buf();
        #[cfg(target_os = "windows")]
        worker_path.push("doom-worker.exe");
        #[cfg(not(target_os = "windows"))]
        worker_path.push("doom-worker");

        // Debug print to see exactly what we are trying to run
        println!("Spawning worker at: {worker_path:?}");

        let child = Command::new(&worker_path) // Use the full validated path
            .arg(format!("127.0.0.1:{port}"))
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to start worker at {worker_path:?}: {e}"));

        let (stream, _) = listener.accept().expect("Worker failed to connect");

        Self::spawn(id, child, stream)
    }

    #[must_use]
    pub fn id(&self) -> i32 {
        self.id
    }

    fn spawn(id: i32, child: Child, stream: std::net::TcpStream) -> Self {
        let (input_tx, input_rx) = std::sync::mpsc::channel::<ToChild>();
        let latest_frame = Arc::new(Mutex::new(None));

        let frame_store = Arc::clone(&latest_frame);

        // Clone the stream: one for the reader thread, one for the writer thread
        let mut reader_stream = stream.try_clone().expect("Failed to clone stream");
        let mut writer_stream = stream;

        // 1. READER THREAD (TCP -> Latest Frame)
        std::thread::spawn(move || {
            use std::io::Read;
            loop {
                let mut len_buf = [0u8; 4];
                if reader_stream.read_exact(&mut len_buf).is_ok() {
                    let len = u32::from_le_bytes(len_buf) as usize;
                    let mut data = vec![0u8; len];
                    if reader_stream.read_exact(&mut data).is_ok() {
                        if let Ok(doom_protocol::ToParent::Frame(frame)) =
                            postcard::from_bytes(&data)
                        {
                            let mut lock = frame_store.lock().unwrap();
                            *lock = Some(frame);
                        }
                    }
                } else {
                    break; // Connection closed
                }
            }
        });

        // 2. WRITER THREAD (Channel -> TCP)
        std::thread::spawn(move || {
            use std::io::Write;
            while let Ok(msg) = input_rx.recv() {
                if let Ok(encoded) = postcard::to_allocvec(&msg) {
                    let _ = writer_stream.write_all(&(encoded.len() as u32).to_le_bytes());
                    let _ = writer_stream.write_all(&encoded);
                    let _ = writer_stream.flush();
                }
            }
        });

        Self {
            id,
            active: true,
            latest_frame,
            input_tx,
            child_handle: child,
            pressed_keys: Vec::new(),
        }
    }

    pub fn set_input(&mut self, input: doom_protocol::Input, pressed: bool) {
        if self.pressed_keys.contains(&input) == pressed {
            return;
        }

        let _ = self.input_tx.send(ToChild::Input { input, pressed });
        if pressed {
            self.pressed_keys.push(input);
        } else {
            self.pressed_keys.retain(|&k| k != input);
        }
    }

    pub fn toggle_input(&mut self, input: doom_protocol::Input) {
        let is_pressed = self.pressed_keys.contains(&input);
        self.set_input(input, !is_pressed);
    }
}

#[derive(Resource, Default)]
pub struct DoomSessionAllocator {
    next_id: i32,
}

impl DoomSessionAllocator {
    #[must_use]
    pub fn create_session(&mut self) -> (DoomSession, ItemStack) {
        let id = self.allocate();
        let mut tag = Compound::new();
        tag.insert("map", id);
        let map = ItemStack::new(ItemKind::FilledMap, 1, Some(tag));
        (DoomSession::from_id(id), map)
    }

    fn allocate(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
