use doom_protocol::packet::{CmdBundle, Frame, Input, TicCmd};
use doom_protocol::{ClientEvent, ServerCommand};
use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use rand::seq::IteratorRandom as _;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::env;
use std::process::Child;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use valence::nbt::Compound;
use valence::prelude::*;

/// Plugin that adds DOOM sessions to the game.
/// Each session runs in a separate worker process.
/// Session IDs are stored as negative to retain normal map functionality
pub struct DoomSessionPlugin;

impl Plugin for DoomSessionPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(DoomSessionRegistry::default())
        .insert_resource(DoomSessionDirectory::default())
        .add_systems(
            Update,
            (
                cleanup_disconnected_clients,
            ),
        );
    }
}

#[derive(Component)]
pub struct DoomSession {
    id: i32,

    /// Whether the session is currently active and running
    pub active: bool,

    /// The latest frame received from the worker
    pub latest_frame: Arc<Mutex<Option<Frame>>>,

    pub received_commands: Arc<Mutex<VecDeque<TicCmd>>>,

    /// Channel to send inputs to the worker
    pub input_tx: IpcSender<ServerCommand>,

    /// Doom worker process handle
    pub child_handle: Child,

    // pub iwad: PathBuf,
    /// Currently pressed keys
    pressed_keys: Vec<Input>,
}

impl DoomSession {
    #[must_use]
    fn from_id(id: i32, wad: &str) -> Self {
        // Create a one-shot server to receive the child's communication channel
        let (server, server_name) = IpcOneShotServer::<IpcSender<ServerCommand>>::new().unwrap();

        let exe_path = env::current_exe().expect("Failed to get current exe path");
        let bin_dir = exe_path.parent().expect("Failed to get bin directory");

        let mut worker_path = bin_dir.to_path_buf();
        #[cfg(target_os = "windows")]
        worker_path.push("doom-worker.exe");
        #[cfg(not(target_os = "windows"))]
        worker_path.push("doom-worker");

        let child = Command::new(&worker_path) // Use the full validated path
            .arg(&server_name)
            .args(["-warp", "1", "1", "-net"])
            // .arg("-player")
            // .arg((id.abs() - 1).to_string())
            .arg("-iwad")
            .arg(wad)
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to start worker at {worker_path:?}: {e}"));

        println!(
            "Created Doom worker with PID {} for session {}",
            child.id(),
            id
        );

        // Wait for the child to connect and send us our command sender
        // This also provides the receiver for HostEvents (Frames)
        let (_, command_tx) = server.accept().unwrap();

        // To get frames and events back from the child, we create 2 different channels
        let (frame_tx, frame_rx) = ipc::channel::<Frame>().unwrap();
        let (event_tx, event_rx) = ipc::channel::<ClientEvent>().unwrap();

        // Register the frame communication channel
        command_tx
            .send(ServerCommand::RegisterPipes { frame_tx, event_tx })
            .ok();

        Self::spawn(id, child, command_tx, frame_rx, event_rx)
    }

    #[must_use]
    pub fn id(&self) -> i32 {
        self.id
    }

    fn spawn(
        id: i32,
        child: Child,
        input_tx: IpcSender<ServerCommand>,
        frame_rx: IpcReceiver<Frame>,
        event_rx: IpcReceiver<ClientEvent>,
    ) -> Self {
        let latest_frame = Arc::new(Mutex::new(None));
        let frame_store = Arc::clone(&latest_frame);

        let received_commands = Arc::new(Mutex::new(VecDeque::with_capacity(128)));
        let command_store = Arc::clone(&received_commands);

        // Reading frames from IPC
        std::thread::spawn(move || {
            // IpcReceiver::recv is blocking, perfect for a dedicated thread
            while let Ok(frame) = frame_rx.recv() {
                let mut lock = frame_store.lock().unwrap();
                *lock = Some(frame);
            }
        });

        // Reading events from IPC
        std::thread::spawn(move || {
            // IpcReceiver::recv is blocking, perfect for a dedicated thread
            while let Ok(event) = event_rx.recv() {
                match event {
                    ClientEvent::TicCmd(tic_cmd) => {
                        let mut lock = command_store.lock().unwrap();
                        lock.push_back(tic_cmd);
                    }
                }
            }
        });

        Self {
            id,
            active: false,
            latest_frame,
            received_commands,
            input_tx,
            child_handle: child,
            pressed_keys: Vec::new(),
        }
    }

    pub fn set_input(&mut self, input: Input, pressed: bool) {
        if !self.active || self.pressed_keys.contains(&input) == pressed {
            return;
        }

        let _ = self.input_tx.send(ServerCommand::Input { input, pressed });
        if pressed {
            self.pressed_keys.push(input);
        } else {
            self.pressed_keys.retain(|&k| k != input);
        }
    }

    pub fn toggle_input(&mut self, input: Input) {
        let is_pressed = self.pressed_keys.contains(&input);
        self.set_input(input, !is_pressed);
    }

    /// Returns true if the active state was changed
    pub fn set_active(&mut self, active: bool) -> bool {
        if self.active == active {
            return false;
        }

        self.active = true;
        let _ = self.input_tx.send(ServerCommand::State { active: true });
        true
    }

    pub fn set_mouse_delta(&mut self, delta: i16) -> bool {
        let _ = self.input_tx.send(ServerCommand::RotationDelta(delta));
        true
    }

    pub fn send_cmd_bundle(&mut self, bundle: CmdBundle) {
        let _ = self.input_tx.send(ServerCommand::NetCmdBundle(bundle));
    }

    pub fn set_local_player(&mut self, id: i32) {
        let _ = self.input_tx.send(ServerCommand::NetSetLocal { id });
    }

    pub fn add_player(&mut self, player_id: i32) {
        let _ = self.input_tx.send(ServerCommand::NetJoin { id: player_id });
    }

    /// Binds a map to view this session
    pub fn bind_map(&self, map: &mut ItemStack) {
        let mut tag = Compound::new();
        tag.insert("map", self.id);
        map.nbt = Some(tag);
    }
}

impl Drop for DoomSession {
    fn drop(&mut self) {
        println!(
            "Killing Doom worker with PID {} for session {}",
            self.child_handle.id(),
            self.id
        );
        let _ = self.child_handle.kill();
    }
}

#[derive(Resource)]
pub struct DoomSessionRegistry {
    next_id: i32,
    free_ids: BinaryHeap<i32>,
    in_use: HashSet<i32>,
}

/// Uses negative map IDs for DOOM sessions
impl DoomSessionRegistry {
    /// Creates a doom session bound to a map
    #[must_use]
    pub fn create_session(&mut self, wad: &str) -> (DoomSession, ItemStack) {
        let id = self.create_id();

        let mut tag = Compound::new();
        tag.insert("map", id);
        let map = ItemStack::new(ItemKind::FilledMap, 1, Some(tag));

        (DoomSession::from_id(id, wad), map)
    }

    /// Creates a doom session and binds it to an existing map
    #[must_use]
    pub fn create_session_with_map(&mut self, map: &mut ItemStack, wad: &str) -> DoomSession {
        let id = self.create_id();

        let mut tag = Compound::new();
        tag.insert("map", id);
        map.nbt = Some(tag);

        DoomSession::from_id(id, wad)
    }

    pub fn replace_session(&mut self, id: i32, wad: &str) -> DoomSession {
        DoomSession::from_id(id, wad)
    }

    pub fn destroy_session(&mut self, session: DoomSession) {
        self.free_id(session.id);
        drop(session);
    }

    /// Creates a map spectating a random session
    #[must_use]
    pub fn create_random_map_view(&mut self) -> Option<ItemStack> {
        let id = *self.in_use.iter().choose(&mut rand::rng())?;
        self.create_map_view(id)
    }

    /// Creates a map spectating a random session
    #[must_use]
    pub fn create_map_view(&mut self, id: i32) -> Option<ItemStack> {
        let mut tag = Compound::new();
        tag.insert("map", id);
        Some(ItemStack::new(ItemKind::FilledMap, 1, Some(tag)))
    }

    /// Used to reserve maps that don't need to be attached to a DOOM session
    /// will prevent the ID from accidentally being reused
    #[must_use]
    pub fn create_id(&mut self) -> i32 {
        let id = self
            .free_ids
            .pop()
            //.map(|Reverse(id)| id)
            .unwrap_or_else(|| {
                let id = self.next_id;
                self.next_id -= 1;
                id
            });

        let inserted = self.in_use.insert(id);
        debug_assert!(inserted, "allocated an id that was already in use: {id}");
        id
    }

    pub fn free_id(&mut self, id: i32) {
        let was_in_use = self.in_use.remove(&id);
        debug_assert!(was_in_use, "free of unknown/not-in-use id: {id}");

        if !was_in_use {
            return;
        }

        self.free_ids.push(id);
    }
}

impl Default for DoomSessionRegistry {
    fn default() -> Self {
        Self {
            next_id: -1,
            free_ids: BinaryHeap::new(),
            in_use: HashSet::new(),
        }
    }
}

/// Allows for keeping track of player-session-player mappings
#[derive(Resource, Default)]
pub struct DoomSessionDirectory {
    pub owner_by_id: HashMap<i32, Entity>,
    pub id_by_owner: HashMap<Entity, i32>,
}

impl DoomSessionDirectory {
    pub fn insert(&mut self, owner: Entity, id: i32) {
        self.owner_by_id.insert(id, owner);
        self.id_by_owner.insert(owner, id);
    }

    pub fn remove_by_owner(&mut self, owner: Entity) -> Option<i32> {
        let id = self.id_by_owner.remove(&owner)?;
        self.owner_by_id.remove(&id);
        Some(id)
    }

    #[must_use]
    pub fn owner_of(&self, id: i32) -> Option<Entity> {
        self.owner_by_id.get(&id).copied()
    }

    #[must_use]
    pub fn id_of(&self, owner: Entity) -> Option<i32> {
        self.id_by_owner.get(&owner).copied()
    }
}

pub fn cleanup_disconnected_clients(
    mut registry: ResMut<DoomSessionRegistry>,
    query: Query<&DoomSession>,
    mut disconnected_clients: RemovedComponents<Client>,
) {
    for entity in disconnected_clients.read() {
        let Ok(session) = query.get(entity) else {
            continue;
        };

        println!("Freeing DOOM session {}", session.id());
        registry.free_id(session.id());
    }
}
