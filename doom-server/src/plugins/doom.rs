use doom_protocol::{Frame, Input, ToChild};
use std::collections::HashSet;
use std::env;
use std::net::TcpListener;
use std::process::Child;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use valence::interact_item::InteractItemEvent;
use valence::inventory::UpdateSelectedSlotEvent;
use valence::math::Vec3Swizzles;
use valence::movement::MovementEvent;
use valence::nbt::Compound;
use valence::prelude::*;
use valence::protocol::packets::play::map_update_s2c::Data;
use valence::protocol::packets::play::MapUpdateS2c;
use valence::protocol::{VarInt, WritePacket};

pub struct DoomPlugin;

impl Plugin for DoomPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(DoomMapAllocator::default())
        .add_systems(
            Update,
            (
                init_clients_with_session,
                cleanup_disconnected_clients,
                freeze_controllers,
                handle_controller_sneak,
                handle_controller_move,
                handle_controller_stop,
                handle_controller_scroll,
                update_all_sessions,
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
    pub input_tx: Sender<ToChild>,

    /// Doom worker process handle
    pub child_handle: Child,

    /// Currently pressed keys
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

        let child = Command::new(&worker_path) // Use the full validated path
            .arg(format!("127.0.0.1:{port}"))
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to start worker at {worker_path:?}: {e}"));

        println!(
            "Created Doom worker with PID {} for session {}",
            child.id(),
            id
        );

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

        // READER THREAD (TCP -> Latest Frame)
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
                    break;
                }
            }
        });

        // WRITER THREAD (Channel -> TCP)
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

    /// Returns true if the active state was changed
    pub fn set_active(&mut self, active: bool) -> bool {
        if self.active == active {
            return false;
        }

        self.active = active;
        let _ = self.input_tx.send(ToChild::State { active });
        true
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
pub struct DoomMapAllocator {
    next_id: i32,
    free_ids: Vec<i32>,
    in_use: HashSet<i32>,
}

/// Uses negative map IDs for DOOM sessions
impl DoomMapAllocator {
    #[must_use]
    pub fn create_session(&mut self) -> (DoomSession, ItemStack) {
        let id = self.alloc();

        let mut tag = Compound::new();
        tag.insert("map", id);
        let map = ItemStack::new(ItemKind::FilledMap, 1, Some(tag));

        (DoomSession::from_id(id), map)
    }

    /// Used to reserve maps that don't need to be attached to a DOOM session
    /// will prevent the ID from accidentally being reused
    #[must_use]
    pub fn alloc(&mut self) -> i32 {
        let id = self.free_ids.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id -= 1;
            id
        });

        let inserted = self.in_use.insert(id);
        debug_assert!(inserted, "allocated an id that was already in use: {id}");
        id
    }

    pub fn free(&mut self, id: i32) {
        let was_in_use = self.in_use.remove(&id);
        debug_assert!(was_in_use, "free of unknown/not-in-use id: {id}");

        if !was_in_use {
            return;
        }

        self.free_ids.push(id);
    }
}

impl Default for DoomMapAllocator {
    fn default() -> Self {
        Self {
            next_id: -1,
            free_ids: Vec::new(),
            in_use: HashSet::new(),
        }
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

fn init_clients_with_session(
    mut commands: Commands,
    mut doom_session_allocator: ResMut<DoomMapAllocator>,
    mut clients: Query<(Entity, &mut Inventory), Added<Client>>,
) {
    for (player, mut inventory) in &mut clients {
        let (doom_session, map) = doom_session_allocator.create_session();
        commands.entity(player).insert(doom_session);
        inventory.set_slot(40, map);
    }
}

pub fn cleanup_disconnected_clients(
    mut alloc: ResMut<DoomMapAllocator>,
    query: Query<&DoomSession>,
    mut disconnected_clients: RemovedComponents<Client>,
) {
    for entity in disconnected_clients.read() {
        let Ok(session) = query.get(entity) else {
            continue;
        };

        println!("Deallocating DOOM session {}", session.id());
        alloc.free(session.id());
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

fn handle_controller_scroll(
    mut commands: Commands,
    mut ev: EventReader<UpdateSelectedSlotEvent>,
    mut q: Query<(Entity, &mut DoomSession, &Inventory, &Position)>,
) {
    for e in &mut ev.read() {
        let Ok((player, mut session, inventory, position)) = q.get_mut(e.client) else {
            continue;
        };

        let active = inventory.slot((e.slot + 36).into()).item == ItemKind::FilledMap;
        let state_changed = session.set_active(active);

        if state_changed {
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

fn update_all_sessions(mut q: Query<(&mut Client, &mut DoomSession)>) {
    for (mut client, session) in &mut q {
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

                client.write_packet(&pkt);
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
