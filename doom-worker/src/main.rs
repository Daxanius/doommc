use doom_protocol::{Frame, GuestCommand, HostEvent};
use doomgeneric::game::DoomGeneric;
use doomgeneric::input::KeyData;
use ipc_channel::ipc::{self, IpcReceiver, IpcSender};
use std::collections::VecDeque;

struct DoomContext {
    command_rx: IpcReceiver<GuestCommand>,
    event_tx: IpcSender<HostEvent>,
    key_queue: VecDeque<KeyData>,
    delta: i16,
    active: bool,
}

impl DoomContext {
    pub fn new(command_rx: IpcReceiver<GuestCommand>, event_tx: IpcSender<HostEvent>) -> Self {
        Self {
            command_rx,
            event_tx,
            key_queue: VecDeque::new(),
            active: false,
            delta: 0,
        }
    }

    fn pump_messages(&mut self) {
        while let Ok(msg) = self.command_rx.try_recv() {
            match msg {
                GuestCommand::Input { input, pressed } => {
                    self.key_queue.push_back(KeyData {
                        pressed,
                        key: input.to_keycode(),
                    });
                }
                GuestCommand::State { active } => self.active = active,
                GuestCommand::RegisterEventPipe { event_tx: _ } => {
                    eprintln!("Attempt to register event pipe after creation!");
                }
                GuestCommand::RotationDelta(rotation) => {
                    self.delta += rotation;
                }
            }
        }
    }

    fn wait_until_resumed(&mut self) {
        while !self.active {
            match self.command_rx.recv() {
                Ok(GuestCommand::State { active }) => self.active = active,
                Ok(GuestCommand::RegisterEventPipe { event_tx: _ }) => {
                    eprintln!("Attempt to register event pipe after creation!");
                }
                Ok(GuestCommand::RotationDelta(rotation)) => {
                    self.delta += rotation;
                }
                Ok(GuestCommand::Input {
                    input: _,
                    pressed: _,
                })
                | Err(_) => (),
            }
        }
    }

    fn send_frame(&mut self, frame: &Frame) {
        let msg = HostEvent::Frame(*frame);

        if let Err(e) = self.event_tx.send(msg) {
            eprintln!("Failed to send frame over IPC: {e}");
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

    fn get_mouse_delta(&mut self) -> i16 {
        let delta = self.delta;
        self.delta = 0;
        delta
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server_name = args.get(1).expect("Missing IPC server name");

    // Create a channel for the parent to send messages TO the child
    let initial_tx: IpcSender<IpcSender<GuestCommand>> =
        IpcSender::connect(server_name.clone()).expect("Could not create IPC connection");
    let (command_tx, command_rx) =
        ipc::channel::<GuestCommand>().expect("Could not create channel");
    initial_tx.send(command_tx).unwrap();

    // BLOCK until the parent sends the Event Pipe (HostEvent sender)
    // This ensures event_tx is ready before DoomContext even exists
    let GuestCommand::RegisterEventPipe { event_tx } = command_rx
        .recv()
        .expect("Failed to receive initial handshake")
    else {
        panic!("Expected RegisterEventPipe as the first message from parent!");
    };

    let context = DoomContext::new(command_rx, event_tx);
    doomgeneric::game::init(context, &args);
    loop {
        doomgeneric::game::tick();
    }
}
