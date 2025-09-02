mod mailbox;

use bevy::{math::DVec3, platform::collections::HashMap, prelude::*};
use bytemuck::{cast_slice, cast_slice_mut};
use std::{
    io::{Cursor, Read, Write},
    mem::size_of,
    net::{SocketAddr, TcpStream},
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    time::Duration,
};

const VRPN_ALIGN: usize = 8;
const VRPN_MAGIC_LENGTH: usize = 16;
const VRPN_COOKIE_SIZE: usize = VRPN_MAGIC_LENGTH + VRPN_ALIGN;
const VRPN_DEFAULT_PORT: u16 = 3883;
const HEADER_SIZE_BYTES: usize = size_of::<MessageHeader>();

const _: () = assert!(HEADER_SIZE_BYTES == 24, "Header size does not match spec!");

type Result<T> = std::io::Result<T>;

fn write_vrpn_cookie(w: &mut impl Write) -> Result<()> {
    // Magic. This roughly corresponds with
    // "vrpn: ver. 07.33  0\x00í\x00\x00\x00"
    // note the use of null
    const COOKIE: [u8; 24] = [
        0x76, 0x72, 0x70, 0x6e, 0x3a, 0x20, 0x76, 0x65, 0x72, 0x2e, 0x20, 0x30, 0x37, 0x2e, 0x33,
        0x33, 0x20, 0x20, 0x30, 0x00, 0xed, 0x00, 0x00, 0x00,
    ];
    w.write_all(&COOKIE)
}

fn check_vrpn_cookie(bytes: &[u8]) -> Option<(u32, u32, u32)> {
    let major = &bytes[11..13];
    let minor = &bytes[14..16];
    let log = bytes[18] as u32;
    const ZERO_ASCII: u32 = 48;

    let major: u32 = std::str::from_utf8(major).ok()?.parse().ok()?;
    let minor: u32 = std::str::from_utf8(minor).ok()?.parse().ok()?;
    let log = log - ZERO_ASCII;
    Some((major, minor, log))
}

fn read_vrpn_cookie(r: &mut impl Read) -> Result<[u8; VRPN_COOKIE_SIZE]> {
    let mut incoming = [0u8; VRPN_COOKIE_SIZE];

    r.read_exact(&mut incoming)?;

    Ok(incoming)
}

fn read_be_f64(r: &mut impl Read) -> std::io::Result<f64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(f64::from_bits(u64::from_be_bytes(b)))
}
fn read_be_f64_n<const N: usize>(r: &mut impl Read) -> std::io::Result<[f64; N]> {
    let mut out = [0.0; N];
    r.read(cast_slice_mut(&mut out))?;
    Ok(out)
}

#[derive(Debug, Default)]
struct MessageHeader([i32; 6]);

fn make_header_for(sender: i32, ty: i32, payload: &[u8]) -> MessageHeader {
    let total_len = payload.len().next_multiple_of(8) + HEADER_SIZE_BYTES;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();

    let ret: [i32; 6] = [
        total_len as i32,
        ts.as_secs() as i32,
        ts.subsec_micros() as i32,
        sender,
        ty,
        0,
    ];

    MessageHeader(ret)
}

/// Write a payload to a byte sink.
fn write_message(dest: &mut impl Write, sender: i32, ty: i32, payload: &[u8]) -> Result<()> {
    let header = make_header_for(sender, ty, payload);

    header.write(dest)?;

    dest.write_all(payload)?;

    // now we need padding to round this up to multiples of 8
    let padding_amount = payload.len().next_multiple_of(8) - payload.len();

    const PADDING: [u8; 16] = [0u8; 16];

    if padding_amount != 0 {
        dest.write_all(&PADDING[0..padding_amount])?;
    }

    Ok(())
}

impl MessageHeader {
    fn read(reader: &mut impl Read) -> Result<Self> {
        let mut h = MessageHeader::default();
        reader.read_exact(cast_slice_mut(&mut h.0[..]))?;
        h.0 = h.0.map(|f| i32::from_be(f));
        Ok(h)
    }

    fn write(&self, writer: &mut impl Write) -> Result<()> {
        let v = self.0.map(|f| f.to_be());
        writer.write_all(&cast_slice(&v))?;

        Ok(())
    }

    #[inline]
    fn packet_length(&self) -> usize {
        self.0[0] as usize
    }

    fn payload_length(&self) -> usize {
        self.packet_length() - size_of::<MessageHeader>()
    }

    fn timestamp(&self) -> (u32, u32) {
        (self.0[1] as u32, self.0[2] as u32)
    }

    fn sender(&self) -> i32 {
        self.0[3]
    }

    fn ty(&self) -> i32 {
        self.0[4]
    }
}

struct ConnectionInfo {
    senders: Vec<String>,
    host: SocketAddr,
}

// =============================================================================

fn get_message(stream: &mut impl Read, buffer: &mut Vec<u8>) -> Result<MessageHeader> {
    //println!("Getting next message...");

    let header = MessageHeader::read(stream)?;

    //println!("Got header...");

    let payload_length = header.payload_length();

    let ceil_length = payload_length.next_multiple_of(VRPN_ALIGN);

    //println!("Read payload {ceil_length}");

    const MAX_PAYLOAD: usize = 64 * 1024;

    if ceil_length > MAX_PAYLOAD {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "payload too large",
        ));
    }

    buffer.resize(ceil_length, 0);

    stream.read(&mut buffer[..])?;

    Ok(header)
}

// =============================================================================

#[derive(Debug)]
struct ComponentInfo {
    id: i32,
    name: String,
    user_id: TrackedItemID,
}

impl Default for ComponentInfo {
    fn default() -> Self {
        Self {
            id: -1,
            name: Default::default(),
            user_id: TrackedItemID(-1),
        }
    }
}

struct VRPNClient {
    remote: TcpStream,
    message_state: MessageState,
}

impl VRPNClient {
    fn new(senders: Vec<String>, host_string: &str, tx: SyncSender<VRPNMessage>) -> Result<Self> {
        let host: SocketAddr = if host_string.contains(':') {
            host_string.parse()
        } else {
            format!("{}:{}", host_string, VRPN_DEFAULT_PORT).parse()
        }
        .unwrap();

        let conn_info = ConnectionInfo { senders, host };

        println!("Start connection...");
        let mut remote = TcpStream::connect_timeout(&conn_info.host, Duration::from_secs(10))?;

        write_vrpn_cookie(&mut remote)?;

        let cookie = read_vrpn_cookie(&mut remote)?;

        let versions = check_vrpn_cookie(&cookie).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Unsupported, "unknown vrpn cookie")
        })?;

        if versions.0 != 7 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "unknown vrpn version",
            ));
        }

        Ok(Self {
            remote,
            message_state: MessageState::new(conn_info.senders, tx.clone()),
        })
    }

    fn service(&mut self) {
        let mut buffer = Vec::new();

        loop {
            let header = get_message(&mut self.remote, &mut buffer);

            let Ok(header) = header else {
                break;
            };

            if self
                .message_state
                .handle_message(header, &buffer, &mut self.remote)
                .is_err()
            {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrackedItemID(pub i32);

struct ComponentList(Vec<Option<ComponentInfo>>);

const MAX_KNOWN_COMPONENTS: usize = 1024;

impl ComponentList {
    fn new() -> Self {
        Self(Vec::with_capacity(128))
    }

    #[inline]
    fn lookup(&self, index: usize) -> Option<&ComponentInfo> {
        self.0.get(index).map(|x| x.as_ref()).flatten()
    }

    #[inline]
    fn lookup_result(&self, index: usize) -> Result<&ComponentInfo> {
        self.lookup(index).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Missing component info")
        })
    }

    fn install(&mut self, index: usize, value: ComponentInfo) -> Option<()> {
        if self.0.get(index).is_some() {
            // already have this, bail
            return None;
        }

        if index >= MAX_KNOWN_COMPONENTS {
            // can't install
            return None;
        }

        // install
        self.0.resize_with(index + 1, || None);
        *(self.0.get_mut(index).expect("we just resized this")) = Some(value);
        Some(())
    }
}

struct MessageState {
    to_watch: HashMap<String, std::sync::Arc<mailbox::Mailbox>>,
    tx: SyncSender<VRPNMessage>,
    remote_sender_list: ComponentList,
    remote_type_list: ComponentList,

    tracker_pos_quat_message_idx: i32,
    tracker_velocity_message_idx: i32,
}

impl MessageState {
    fn new(to_watch: Vec<String>, tx: SyncSender<VRPNMessage>) -> Self {
        let remote_sender_list = ComponentList::new();
        let remote_type_list = ComponentList::new();

        Self {
            to_watch,
            tx,
            remote_sender_list,
            remote_type_list,
            tracker_pos_quat_message_idx: -100,
            tracker_velocity_message_idx: -101,
        }
    }

    fn produce(&mut self, m: VRPNMessage) -> Result<()> {
        let result = self.tx.try_send(m);

        use std::sync::mpsc::TrySendError;

        match result {
            Err(TrySendError::Full(_)) => {
                //println!("Dropping VRPN message...too fast!");
                // discard.. for now
            }
            Err(TrySendError::Disconnected(_)) => {
                // we are done
                return Ok(());
            }
            _ => {}
        }

        return Ok(());
    }

    fn handle_pos_quat(&mut self, sender: usize, source: &mut impl Read) -> Result<()> {
        // we need to skip a double here for the timestamp?
        let _dummy = read_be_f64(source)?;

        let pos_and_quat: [f64; 7] = read_be_f64_n(source)?;

        self.produce(VRPNMessage::Pos(
            self.remote_sender_list.lookup_result(sender)?.user_id,
            PositionRotation {
                pos: transform_position(pos_and_quat[0..3].try_into().unwrap()),
                rot: transform_rotation(pos_and_quat[3..].try_into().unwrap()),
            },
        ))
        // println!(
        //     "{}: Pos {:?}",
        //     self.remote_sender_list[sender].name, pos_and_quat
        // );
    }

    fn handle_velocity(&mut self, sender: usize, source: &mut impl Read) -> Result<()> {
        // we need to skip a double here for the timestamp?
        let _dummy = read_be_f64(source)?;

        let velocities: [f64; 8] = read_be_f64_n(source)?;

        self.produce(VRPNMessage::Vel(
            self.remote_sender_list.lookup_result(sender)?.user_id,
            Velocity {
                pos: velocities[0..3].try_into().unwrap(),
            },
        ))

        // println!(
        //     "{}: Vel {:?}",
        //     self.remote_sender_list[sender].name, velocities
        // );
    }

    fn handle_message(
        &mut self,
        header: MessageHeader,
        payload: &Vec<u8>,
        output: &mut impl Write,
    ) -> Result<()> {
        //println!("Dispatch message {}", header.ty());
        const TYPE_SENDER: i32 = -1;
        const TYPE_CONNTYPE: i32 = -2;
        const TYPE_UDPDESC: i32 = -3;
        const TYPE_LOGDESC: i32 = -4;
        const TYPE_DISCONN: i32 = -5;

        // we need a type
        // if type >= 0 this is a 'user handler'
        // else this is a system id

        let mut cursor = Cursor::new(payload);

        fn extract_i32(c: &mut impl Read) -> i32 {
            let mut ret = [0u8; 4];
            c.read(&mut ret).unwrap();
            i32::from_be_bytes(ret)
        }

        fn extract_prefixed_string(c: &mut impl Read) -> Result<String> {
            println!("Extract NAME");
            let len = extract_i32(c);
            println!("LEN: {len}");
            // apparently the names are encoded with length + 1
            // now, we might not care, because no other string comes after...
            // the strings DO include a null, so we need to strip that

            if len <= 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bad length",
                ));
            }

            let mut bytes = vec![0u8; len as usize];
            c.read_exact(&mut bytes)?;

            // strip nul
            if let Some(&0) = bytes.last() {
                bytes.pop();
            }

            String::from_utf8(bytes)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad string"))
        }

        match header.ty() {
            v if v == self.tracker_pos_quat_message_idx => {
                self.handle_pos_quat(header.sender() as usize, &mut cursor)
            }
            v if v == self.tracker_velocity_message_idx => {
                self.handle_velocity(header.sender() as usize, &mut cursor)
            }
            TYPE_SENDER => {
                let sender_id = header.sender();
                let sender_name = extract_prefixed_string(&mut cursor)?;
                println!("New sender {sender_id}: '{sender_name}'");

                let bounce =
                    sender_name.starts_with("VRPN Control") || self.to_watch.contains(&sender_name);

                self.remote_sender_list.install(
                    sender_id as usize,
                    ComponentInfo {
                        id: sender_id,
                        name: sender_name.clone(),
                        user_id: TrackedItemID(
                            self.to_watch
                                .iter()
                                .position(|f| f == &sender_name)
                                .map(|f| f as i32)
                                .unwrap_or(-1),
                        ),
                    },
                );

                if bounce {
                    return write_message(output, sender_id, TYPE_SENDER, payload);
                }

                Ok(())
            }
            TYPE_CONNTYPE => {
                let type_id = header.sender();
                let type_name = extract_prefixed_string(&mut cursor)?;
                println!("New type {type_id}: '{type_name}'");

                match type_name.as_str() {
                    "vrpn_Tracker Pos_Quat" => {
                        self.tracker_pos_quat_message_idx = type_id;
                        println!("Recognising pos quat message as {type_id}");
                    }
                    "vrpn_Tracker Velocity" => {
                        self.tracker_velocity_message_idx = type_id;
                        println!("Recognising velocity message as {type_id}");
                    }
                    _ => {
                        println!("Unrecognised name '{}'", type_name.as_str())
                    }
                }

                self.remote_type_list.install(
                    type_id as usize,
                    ComponentInfo {
                        id: type_id,
                        name: type_name,
                        user_id: TrackedItemID(-1),
                    },
                );

                // bounce all types by default
                write_message(output, type_id, TYPE_CONNTYPE, payload)
            }
            TYPE_UDPDESC => {
                println!("Skipping UDP description");
                return Ok(());
            }
            TYPE_LOGDESC => {
                println!("Skipping Log description");
                return Ok(());
            }
            TYPE_DISCONN => {
                // Docs say this will never be sent over the wire...
                println!("Skipping disconn description");
                return Ok(());
            }
            _ => {
                //println!("Unknown message type: {x}");
                return Ok(());
            }
        }
    }
}

// Bevy Side =============================================================================

fn transform_position(p: [f64; 3]) -> [f64; 3] {
    [-p[0], p[2], p[1]]
}

fn transform_rotation(p: [f64; 4]) -> [f64; 4] {
    [-p[0], p[2], p[1], p[3]]
}

fn vrpn_spinner(senders: Vec<String>, host_string: String, tx: SyncSender<VRPNMessage>) {
    let mut state = VRPNClient::new(senders, &host_string, tx).expect("unable to connect");

    state.service();
}

pub fn start_vrpn_client(senders: Vec<String>, host_string: &str) -> VRPNResource {
    let (tx, rx) = sync_channel(32);

    let host_string = host_string.to_owned();

    let handle = std::thread::spawn(|| {
        vrpn_spinner(senders, host_string, tx);
    });

    VRPNResource {
        vrpn_thread: handle,
        rx,
    }
}

#[derive(Debug)]
pub enum VRPNMessage {
    Pos(TrackedItemID, PositionRotation),
    Vel(TrackedItemID, Velocity),
}

#[derive(Debug)]
pub struct PositionRotation {
    pub pos: [f64; 3],
    pub rot: [f64; 4],
}

#[derive(Debug)]
pub struct Velocity {
    pub pos: [f64; 3],
}

pub struct VRPNResource {
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    vrpn_thread: Option<std::thread::JoinHandle<()>>,
    rx: Receiver<VRPNMessage>,
}

impl VRPNResource {
    pub fn for_all_message(&mut self, mut f: impl FnMut(VRPNMessage)) {
        loop {
            let message = self.rx.try_recv();

            let Ok(message) = message else {
                return;
            };

            f(message);
        }
    }

    pub fn wait_for_shutdown(self) {
        drop(self.rx);
        self.vrpn_thread.join().unwrap();
    }
}

#[derive(Component)]
pub struct VRPNLink {
    pub sender: String,
    pub host: String,
    pub port: u16,
}

impl VRPNLink {
    pub fn new(sender: String, host: String, port: u16) -> Self {
        Self { sender, host, port }
    }
}

#[derive(Component)]
struct VRPNLinkConnected {
    reader: std::sync::Mutex<VRPNResource>,
}

fn check_for_new_vrpn(
    mut commands: Commands,
    query: Query<(Entity, &VRPNLink), Without<VRPNLinkConnected>>,
) {
    for (e, l) in query.iter() {
        let reader = start_vrpn_client(vec![l.sender.clone()], &format!("{}:{}", l.host, l.port));

        println!("Creating new VRPN reader!");

        commands.entity(e).insert(VRPNLinkConnected {
            reader: std::sync::Mutex::new(reader),
        });
    }
}

fn service_vrpn(
    mut commands: Commands,
    mut query: Query<(Entity, &VRPNLinkConnected, &mut Transform)>,
) {
    use std::sync::mpsc::TryRecvError;

    for (e, c, mut tf) in query.iter_mut() {
        let lock = c.reader.lock().unwrap();

        loop {
            match lock.rx.try_recv() {
                Ok(message) => match message {
                    VRPNMessage::Pos(_, position_rotation) => {
                        let p: DVec3 = position_rotation.pos.into();
                        tf.translation = p.as_vec3();
                    }
                    _ => {}
                },
                Err(x) => match x {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => {
                        commands.entity(e).remove::<VRPNLinkConnected>();
                    }
                },
            }
        }
    }
}

pub struct VRPNPlugin;

impl Plugin for VRPNPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (check_for_new_vrpn, service_vrpn));
    }
}

#[cfg(test)]
mod test {
    //use std::io::Cursor;

    //use super::{MessageState, check_vrpn_cookie, get_message};
    use super::check_vrpn_cookie;

    #[test]
    fn check_cookie() {
        const COOKIE: [u8; 24] = [
            0x76, 0x72, 0x70, 0x6e, 0x3a, 0x20, 0x76, 0x65, 0x72, 0x2e, 0x20, 0x30, 0x37, 0x2e,
            0x33, 0x33, 0x20, 0x20, 0x30, 0x00, 0xed, 0x00, 0x00, 0x00,
        ];

        assert_eq!(check_vrpn_cookie(&COOKIE).unwrap(), (7, 33, 0));
    }

    #[test]
    fn test_read() {
        // let data = std::fs::read("assets/vrpn/dump_setup.bin").unwrap();
        // let mut src_cursor = Cursor::new(data);

        // let mut buffer = Vec::new();
        // let mut state = MessageState::new(vec!["Head0".into()]);

        // let mut output = Vec::new();

        // loop {
        //     let header = get_message(&mut src_cursor, &mut buffer);

        //     state.handle_message(header, &buffer, &mut output);
        // }
    }
}
