use doom_protocol::{Frame, GuestCommand, HostEvent, Input};
use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use rand::seq::IteratorRandom as _;
use std::collections::{HashMap, HashSet};
use std::env;
use std::process::Child;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use valence::interact_item::InteractItemEvent;
use valence::math::Vec3Swizzles;
use valence::movement::MovementEvent;
use valence::nbt::Compound;
use valence::prelude::*;
use valence::protocol::packets::play::map_update_s2c::Data;
use valence::protocol::packets::play::MapUpdateS2c;
use valence::protocol::{VarInt, WritePacket};

use crate::extensions::inventory::InventoryExt;
use crate::extensions::item::ItemStackExt;
use crate::extensions::nbt::NbtValue;
use crate::plugins::hotbar::SelectedHotbarSlot;

/// Plugin that adds DOOM sessions to players holding a filled map in their
/// inventory hotbar. Each session runs in a separate worker process.
/// The map ID is negative and corresponds to the session ID.
pub struct DoomPlugin;

impl Plugin for DoomPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(DoomSessionRegistry::default())
        .insert_resource(DoomSessionDirectory::default())
        .add_systems(
            Update,
            (
                cleanup_disconnected_clients,
                freeze_controllers,
                handle_controller_sneak,
                handle_controller_move,
                handle_controller_stop,
                update_session_active_from_held_item,
                update_active_sessions,
                handle_controller_interact,
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

    /// Channel to send inputs to the worker
    pub input_tx: IpcSender<GuestCommand>,

    /// Doom worker process handle
    pub child_handle: Child,

    // pub iwad: PathBuf,
    /// Currently pressed keys
    pressed_keys: Vec<doom_protocol::Input>,
}

impl DoomSession {
    #[must_use]
    fn from_id(id: i32, wad: &str) -> Self {
        // Create a one-shot server to receive the child's communication channel
        let (server, server_name) = IpcOneShotServer::<IpcSender<GuestCommand>>::new().unwrap();

        let exe_path = env::current_exe().expect("Failed to get current exe path");
        let bin_dir = exe_path.parent().expect("Failed to get bin directory");

        let mut worker_path = bin_dir.to_path_buf();
        #[cfg(target_os = "windows")]
        worker_path.push("doom-worker.exe");
        #[cfg(not(target_os = "windows"))]
        worker_path.push("doom-worker");

        let child = Command::new(&worker_path) // Use the full validated path
            .arg(&server_name)
            .args(["-warp", "1", "1"])
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

        // To get frames BACK from the child, we create a channel here
        let (event_tx, event_rx) = ipc::channel::<HostEvent>().unwrap();

        // Send this event_tx to the child so it knows where to send frames
        // (Assuming you've updated your worker's logic to receive this)
        command_tx
            .send(GuestCommand::RegisterEventPipe { event_tx })
            .ok();

        Self::spawn(id, child, command_tx, event_rx)
    }

    #[must_use]
    pub fn id(&self) -> i32 {
        self.id
    }

    fn spawn(
        id: i32,
        child: Child,
        input_tx: IpcSender<GuestCommand>,
        event_rx: IpcReceiver<HostEvent>,
    ) -> Self {
        let latest_frame = Arc::new(Mutex::new(None));
        let frame_store = Arc::clone(&latest_frame);

        // ONLY ONE THREAD NEEDED: Reading frames from IPC
        std::thread::spawn(move || {
            // IpcReceiver::recv is blocking, perfect for a dedicated thread
            while let Ok(event) = event_rx.recv() {
                match event {
                    HostEvent::Frame(frame) => {
                        let mut lock = frame_store.lock().unwrap();
                        *lock = Some(frame);
                    }
                }
            }
        });

        Self {
            id,
            active: false,
            latest_frame,
            input_tx,
            child_handle: child,
            pressed_keys: Vec::new(),
        }
    }

    pub fn set_input(&mut self, input: doom_protocol::Input, pressed: bool) {
        if !self.active || self.pressed_keys.contains(&input) == pressed {
            return;
        }

        let _ = self.input_tx.send(GuestCommand::Input { input, pressed });
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

    /// Returns true if the active state was changed
    pub fn set_active(&mut self, active: bool) -> bool {
        if self.active == active {
            return false;
        }

        self.active = active;
        let _ = self.input_tx.send(GuestCommand::State { active });
        true
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
    free_ids: Vec<i32>,
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
        let id = self.free_ids.pop().unwrap_or_else(|| {
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
            free_ids: Vec::new(),
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TurnState {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveState {
    #[default]
    None,
    Forward,
    Back,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrafeState {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Component)]
pub struct DoomController {
    pub last_tick: i64,
    pub last_yaw: f32,
    pub turn_state: TurnState,
    pub move_state: MoveState,
    pub strafe_state: StrafeState,
    pub freeze_position: DVec3,
}

impl Default for DoomController {
    fn default() -> Self {
        Self {
            last_tick: 0,
            last_yaw: 0.0,
            turn_state: TurnState::None,
            move_state: MoveState::None,
            strafe_state: StrafeState::None,
            freeze_position: DVec3::ZERO,
        }
    }
}

impl DoomController {
    #[must_use]
    pub fn new(freeze_position: DVec3) -> Self {
        Self {
            freeze_position,
            ..Default::default()
        }
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

fn freeze_controllers(mut q: Query<(&mut Position, &DoomController)>) {
    for (mut position, controller) in &mut q {
        position.0 = controller.freeze_position;
    }
}

fn handle_controller_stop(server: Res<Server>, mut q: Query<(&mut DoomSession, &DoomController)>) {
    let current_tick = server.current_tick();

    for (mut session, controller) in &mut q {
        if current_tick > controller.last_tick + 10 {
            session.set_input(Input::Up, false);
            session.set_input(Input::Down, false);
            session.set_input(Input::Left, false);
            session.set_input(Input::Right, false);
        }
    }
}

fn handle_controller_move(
    mut ev: EventReader<MovementEvent>,
    server: Res<Server>,
    mut q: Query<(&mut DoomSession, &mut DoomController)>,
) {
    const TURN_ON: f32 = 3.0;
    const TURN_OFF: f32 = 1.5;
    const MOVE_ON: f64 = 0.03;
    const MOVE_OFF: f64 = 0.015;

    for e in ev.read() {
        let Ok((mut session, mut tr)) = q.get_mut(e.client) else {
            continue;
        };

        // turn from yaw delta
        let dyaw = wrap_degrees(e.look.yaw - tr.last_yaw);

        let turn_right = if tr.turn_state == TurnState::Right {
            dyaw > TURN_OFF
        } else {
            dyaw > TURN_ON
        };
        let turn_left = if tr.turn_state == TurnState::Left {
            dyaw < -TURN_OFF
        } else {
            dyaw < -TURN_ON
        };

        tr.turn_state = if turn_right {
            TurnState::Right
        } else if turn_left {
            TurnState::Left
        } else {
            TurnState::None
        };

        session.set_input(Input::Right, turn_right);
        session.set_input(Input::Left, turn_left);

        tr.last_yaw = e.look.yaw;

        // movement intent from delta
        let delta = e.position - e.old_position;

        let yaw = e.look.yaw.to_radians();
        let forward = Vec2::new(-yaw.sin(), yaw.cos()).normalize();
        let right = -Vec2::new(yaw.cos(), yaw.sin()).normalize();

        let f = delta.xz().dot(forward.as_dvec2());
        let r = delta.xz().dot(right.as_dvec2());

        let forward_on = if tr.move_state == MoveState::Forward {
            f > MOVE_OFF
        } else {
            f > MOVE_ON
        };
        let back_on = if tr.move_state == MoveState::Back {
            f < -MOVE_OFF
        } else {
            f < -MOVE_ON
        };

        tr.move_state = if forward_on {
            MoveState::Forward
        } else if back_on {
            MoveState::Back
        } else {
            MoveState::None
        };

        session.set_input(Input::Up, forward_on);
        session.set_input(Input::Down, back_on);

        let strafe_r = if tr.strafe_state == StrafeState::Right {
            r > MOVE_OFF
        } else {
            r > MOVE_ON
        };
        let strafe_l = if tr.strafe_state == StrafeState::Left {
            r < -MOVE_OFF
        } else {
            r < -MOVE_ON
        };

        tr.strafe_state = if strafe_r {
            StrafeState::Right
        } else if strafe_l {
            StrafeState::Left
        } else {
            StrafeState::None
        };

        session.set_input(Input::StrafeRight, strafe_r);
        session.set_input(Input::StrafeLeft, strafe_l);

        tr.last_tick = server.current_tick();
    }
}

fn handle_controller_sneak(mut ev: EventReader<SneakEvent>, mut q: Query<&mut DoomSession>) {
    for e in &mut ev.read() {
        let Ok(mut session) = q.get_mut(e.client) else {
            continue;
        };

        let down = e.state == SneakState::Start;
        session.set_input(Input::Shoot, down);
        session.set_input(Input::Enter, down);
    }
}

fn handle_controller_interact(
    mut ev: EventReader<InteractItemEvent>,
    mut q: Query<&mut DoomSession>,
) {
    for e in &mut ev.read() {
        let Ok(mut session) = q.get_mut(e.client) else {
            continue;
        };

        session.toggle_input(Input::Use);
    }
}

fn update_session_active_from_held_item(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut DoomSession,
        &Inventory,
        &Position,
        &SelectedHotbarSlot,
    )>,
) {
    for (player, mut session, inventory, position, selected) in &mut q {
        let active = should_be_active(session.id, inventory, selected.0);

        if session.set_active(active) {
            if active {
                commands
                    .entity(player)
                    .insert(DoomController::new(position.0));
            } else {
                commands.entity(player).remove::<DoomController>();
            }
        }
    }
}

fn update_active_sessions(
    mut q: Query<&mut DoomSession>,
    mut clients: Query<(&mut Client, &Inventory, &SelectedHotbarSlot)>,
) {
    for session in &mut q {
        if !session.active {
            continue;
        }

        if let Ok(mut frame_lock) = session.latest_frame.try_lock() {
            if let Some(frame) = frame_lock.take() {
                let pkt = MapUpdateS2c {
                    map_id: VarInt(session.id()),
                    scale: 0,
                    locked: true,
                    icons: None,
                    data: Some(Data {
                        columns: 128,
                        rows: 128,
                        position: [0, 0],
                        data: &frame.0,
                    }),
                };

                for (mut client, inventory, selected_slot) in &mut clients {
                    if should_be_active(session.id(), inventory, selected_slot.0) {
                        client.write_packet(&pkt);
                    }
                }
            }
        }
    }
}

fn wrap_degrees(mut d: f32) -> f32 {
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

fn should_be_active(session_id: i32, inventory: &Inventory, selected: u8) -> bool {
    let stack = inventory.hotbar_slot(selected);
    stack.is_kind(ItemKind::FilledMap) && stack.has_tag_with("map", &NbtValue::from(session_id).0)
}
