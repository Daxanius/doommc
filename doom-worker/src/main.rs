use doom_protocol::{
    ClientEvent, ServerCommand,
    packet::{Frame, TicCmd},
};
use doom_worker::utils::{packet_to_ticcmd, ticcmds_to_bundle};
use doomgeneric::{
    client::{DG_CL_RemovePlayer, MAX_PLAYERS},
    game::DoomGeneric,
};
use doomgeneric::{
    client::{DG_CL_SetCmdBundle, DG_CL_SpawnPlayer},
    input::KeyData,
};
use ipc_channel::ipc::{self, IpcReceiver, IpcSender};
use std::collections::VecDeque;

struct DoomContext {
    command_rx: IpcReceiver<ServerCommand>,
    frame_tx: IpcSender<Frame>,
    event_tx: IpcSender<ClientEvent>,
    key_queue: VecDeque<KeyData>,
    recv_player_cmds: Vec<TicCmd>,
    has_player_cmds: bool,
    delta: i16,
    active: bool,
}

impl DoomContext {
    pub fn new(
        command_rx: IpcReceiver<ServerCommand>,
        frame_tx: IpcSender<Frame>,
        event_tx: IpcSender<ClientEvent>,
    ) -> Self {
        Self {
            command_rx,
            frame_tx,
            event_tx,
            key_queue: VecDeque::new(),
            recv_player_cmds: Vec::with_capacity(MAX_PLAYERS),
            has_player_cmds: false,
            active: false,
            delta: 0,
        }
    }

    fn pump_messages(&mut self) {
        while let Ok(msg) = self.command_rx.try_recv() {
            match msg {
                ServerCommand::Input { input, pressed } => {
                    self.key_queue.push_back(KeyData {
                        pressed,
                        key: input.to_keycode(),
                    });
                }
                ServerCommand::State { active } => self.active = active,
                ServerCommand::RegisterPipes {
                    frame_tx: _,
                    event_tx: _,
                } => {
                    eprintln!("Attempt to register event pipe after creation!");
                }
                ServerCommand::RotationDelta(rotation) => {
                    self.delta += rotation;
                }
                ServerCommand::NetCmdBundle(cmds) => {
                    self.recv_player_cmds = cmds;
                    self.has_player_cmds = true;
                }
                ServerCommand::NetJoin { id } => unsafe { DG_CL_SpawnPlayer(id) },
                ServerCommand::NetLeave { id } => unsafe { DG_CL_RemovePlayer(id) },
            }
        }
    }

    fn wait_until_resumed(&mut self) {
        while !self.active {
            match self.command_rx.recv() {
                Ok(ServerCommand::State { active }) => self.active = active,
                Ok(ServerCommand::RegisterPipes {
                    frame_tx: _,
                    event_tx: _,
                }) => {
                    eprintln!("Attempt to register event pipe after creation!");
                }
                Ok(ServerCommand::NetCmdBundle(cmds)) => {
                    self.recv_player_cmds = cmds;
                    self.has_player_cmds = true;
                }
                Ok(ServerCommand::NetJoin { id }) => unsafe { DG_CL_SpawnPlayer(id) },
                Ok(ServerCommand::NetLeave { id }) => unsafe { DG_CL_RemovePlayer(id) },
                Ok(
                    ServerCommand::Input {
                        input: _,
                        pressed: _,
                    }
                    | _,
                )
                | Err(_) => (),
            }
        }
    }

    fn wait_until_tick(&mut self) {
        while !self.has_player_cmds {
            match self.command_rx.recv() {
                Ok(ServerCommand::State { active }) => self.active = active,
                Ok(ServerCommand::RegisterPipes {
                    frame_tx: _,
                    event_tx: _,
                }) => {
                    eprintln!("Attempt to register event pipe after creation!");
                }
                Ok(ServerCommand::NetCmdBundle(cmds)) => {
                    self.recv_player_cmds = cmds;
                    self.has_player_cmds = true;
                }
                Ok(ServerCommand::NetJoin { id }) => unsafe { DG_CL_SpawnPlayer(id) },
                Ok(ServerCommand::NetLeave { id }) => unsafe { DG_CL_RemovePlayer(id) },
                Ok(
                    ServerCommand::Input {
                        input: _,
                        pressed: _,
                    }
                    | _,
                )
                | Err(_) => (),
            }
        }
    }

    fn send_frame(&mut self, frame: &Frame) {
        if let Err(e) = self.frame_tx.send(*frame) {
            eprintln!("Failed to send frame over IPC: {e}");
        }
    }
}

impl DoomGeneric for DoomContext {
    fn draw_frame(&mut self, screen_buffer: &[u8], xres: usize, yres: usize) {
        self.pump_messages();
        self.wait_until_tick();

        if !self.active {
            self.wait_until_resumed();
        }

        let (cmds, mask) = ticcmds_to_bundle(&self.recv_player_cmds);
        unsafe { DG_CL_SetCmdBundle(cmds.as_ptr(), mask.as_ptr()) };

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

    fn get_settings(&mut self, settings: &mut doomgeneric::client::DoomGameSettingsRaw) {}

    fn send_tic_cmd(
        &mut self,
        cmd: &doomgeneric::client::DoomInputPacketRaw,
        maketic: i32,
        player_id: i32,
    ) {
        let _ = self.event_tx.send(ClientEvent::TicCmd(packet_to_ticcmd(
            cmd, maketic, player_id,
        )));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server_name = args.get(1).expect("Missing IPC server name");

    // Create a channel for the parent to send messages TO the child
    let initial_tx: IpcSender<IpcSender<ServerCommand>> =
        IpcSender::connect(server_name.clone()).expect("Could not create IPC connection");
    let (command_tx, command_rx) =
        ipc::channel::<ServerCommand>().expect("Could not create channel");
    initial_tx.send(command_tx).unwrap();

    // BLOCK until the parent sends the Event Pipe (HostEvent sender)
    // This ensures event_tx is ready before DoomContext even exists
    let ServerCommand::RegisterPipes { frame_tx, event_tx } = command_rx
        .recv()
        .expect("Failed to receive initial handshake")
    else {
        panic!("Expected RegisterPipes as the first message from parent!");
    };

    let context = DoomContext::new(command_rx, frame_tx, event_tx);
    doomgeneric::game::init(context, &args);
    loop {
        doomgeneric::game::tick();
    }
}
