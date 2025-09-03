mod mailbox;

use bevy::{
    math::{DQuat, DVec3},
    platform::collections::HashMap,
    prelude::*,
};
use bytemuck::{cast_slice, cast_slice_mut};
use std::{
    io::{Cursor, Read, Write},
    mem::size_of,
    net::{SocketAddr, TcpStream},
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
    r.read_exact(cast_slice_mut(&mut out))?;
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

    // fn timestamp(&self) -> (u32, u32) {
    //     (self.0[1] as u32, self.0[2] as u32)
    // }

    fn sender(&self) -> i32 {
        self.0[3]
    }

    fn ty(&self) -> i32 {
        self.0[4]
    }
}

struct ConnectionInfo {
    senders: HashMap<String, SharedItemState>,
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

    stream.read_exact(&mut buffer[..])?;

    Ok(header)
}

// =============================================================================

type SharedItemState = std::sync::Arc<mailbox::Mailbox<ItemState>>;

struct SenderInfo {
    id: i32,
    name: String,
}

struct TypeInfo {
    id: i32,
    name: String,
}

struct VRPNClient {
    remote: TcpStream,
    message_state: MessageState,
}

impl VRPNClient {
    fn new(to_watch: HashMap<String, SharedItemState>, host_string: &str) -> Result<Self> {
        let host: SocketAddr = if host_string.contains(':') {
            host_string.parse()
        } else {
            format!("{}:{}", host_string, VRPN_DEFAULT_PORT).parse()
        }
        .unwrap();

        let conn_info = ConnectionInfo {
            senders: to_watch,
            host,
        };

        trace!("Start connection...");
        let mut remote = TcpStream::connect_timeout(&conn_info.host, Duration::from_secs(10))?;

        remote.set_read_timeout(Some(Duration::from_millis(100)))?;

        write_vrpn_cookie(&mut remote)?;

        let cookie = read_vrpn_cookie(&mut remote)?;

        let versions = check_vrpn_cookie(&cookie).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Unsupported, "unknown vrpn cookie")
        })?;

        if versions.0 != 7 {
            warn!("Unknown VRPN version!");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "unknown vrpn version",
            ));
        }

        Ok(Self {
            remote,
            message_state: MessageState::new(conn_info.senders),
        })
    }

    fn service(&mut self, shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        let mut buffer = Vec::new();

        while shutdown.load(std::sync::atomic::Ordering::Acquire) {
            match get_message(&mut self.remote, &mut buffer) {
                Ok(header) => {
                    if self
                        .message_state
                        .handle_message(header, &buffer, &mut self.remote)
                        .is_err()
                    {
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(_) => break,
            }
        }
    }
}

// MARK: Sender Lists

struct SenderList {
    infos: Vec<Option<SenderInfo>>,
    to_watch: HashMap<String, SharedItemState>,
    watched: HashMap<usize, SharedItemState>,
}

const MAX_KNOWN_COMPONENTS: usize = 1024;

impl SenderList {
    fn new(to_watch: HashMap<String, SharedItemState>) -> Self {
        Self {
            infos: Vec::with_capacity(128),
            to_watch,
            watched: Default::default(),
        }
    }

    fn is_watching(&self, name: &str) -> bool {
        self.to_watch.contains_key(name)
    }

    #[inline]
    fn lookup(&self, index: usize) -> Option<&SharedItemState> {
        self.watched.get(&index)
    }

    #[inline]
    fn lookup_result(&self, index: usize) -> Result<&SharedItemState> {
        self.lookup(index).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Missing component info")
        })
    }

    fn install(&mut self, index: usize, name: String) -> Option<()> {
        if matches!(self.infos.get(index), Some(Some(_))) {
            return None;
        }

        if index >= MAX_KNOWN_COMPONENTS {
            // can't install
            warn!("overflow! dropping component");
            return None;
        }

        // install

        if let Some(w) = self.to_watch.get(&name) {
            self.watched.insert(index, w.clone());
        }

        self.infos.resize_with(index + 1, || None);

        *(self.infos.get_mut(index).expect("we just resized this")) = Some(SenderInfo {
            id: index as i32,
            name,
        });

        Some(())
    }
}

struct TypeList(Vec<Option<TypeInfo>>);

impl TypeList {
    fn new() -> Self {
        Self(Vec::with_capacity(128))
    }

    #[inline]
    fn lookup(&self, index: usize) -> Option<&TypeInfo> {
        self.0.get(index).map(|x| x.as_ref()).flatten()
    }

    #[inline]
    fn lookup_result(&self, index: usize) -> Result<&TypeInfo> {
        self.lookup(index).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Missing component info")
        })
    }

    fn install(&mut self, index: usize, value: TypeInfo) -> Option<()> {
        if matches!(self.0.get(index), Some(Some(_))) {
            return None;
        }

        if index >= MAX_KNOWN_COMPONENTS {
            // can't install
            warn!("overflow! dropping component");
            return None;
        }

        // install
        self.0.resize_with(index + 1, || None);
        *(self.0.get_mut(index).expect("we just resized this")) = Some(value);
        Some(())
    }
}

// MARK: Message State

#[derive(Debug, Default, Clone, Copy)]
pub struct ItemState {
    position: DVec3,
    rotation: DQuat,
}

struct MessageState {
    remote_sender_list: SenderList,
    remote_type_list: TypeList,

    tracker_pos_quat_message_idx: i32,
}

impl MessageState {
    fn new(to_watch: HashMap<String, SharedItemState>) -> Self {
        let remote_sender_list = SenderList::new(to_watch);
        let remote_type_list = TypeList::new();

        Self {
            remote_sender_list,
            remote_type_list,
            tracker_pos_quat_message_idx: -100,
        }
    }

    fn handle_pos_quat(&mut self, sender: usize, source: &mut impl Read) -> Result<()> {
        // we need to skip a double here for the timestamp?
        let _dummy = read_be_f64(source)?;

        let pos_and_quat: [f64; 7] = read_be_f64_n(source)?;

        self.remote_sender_list
            .lookup_result(sender)?
            .write(ItemState {
                position: transform_position(pos_and_quat[0..3].try_into().unwrap()).into(),
                rotation: transform_rotation(pos_and_quat[3..].try_into().unwrap()).into(),
            });
        // println!(
        //     "{}: Pos {:?}",
        //     self.remote_sender_list[sender].name, pos_and_quat
        // );

        Ok(())
    }

    fn handle_message(
        &mut self,
        header: MessageHeader,
        payload: &[u8],
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

        fn extract_i32(c: &mut impl Read) -> Result<i32> {
            let mut ret = [0u8; 4];
            c.read_exact(&mut ret)?;
            Ok(i32::from_be_bytes(ret))
        }

        fn extract_prefixed_string(c: &mut impl Read) -> Result<String> {
            trace!("Extract NAME");
            let len = extract_i32(c)?;
            trace!("LEN: {len}");
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
            TYPE_SENDER => {
                let sender_id = header.sender();
                let sender_name = extract_prefixed_string(&mut cursor)?;
                trace!("New sender {sender_id}: '{sender_name}'");

                let bounce = sender_name.starts_with("VRPN Control")
                    || self.remote_sender_list.is_watching(&sender_name);

                self.remote_sender_list
                    .install(sender_id as usize, sender_name.clone());

                if bounce {
                    return write_message(output, sender_id, TYPE_SENDER, payload);
                }

                Ok(())
            }
            TYPE_CONNTYPE => {
                let type_id = header.sender();
                let type_name = extract_prefixed_string(&mut cursor)?;
                trace!("New type {type_id}: '{type_name}'");

                match type_name.as_str() {
                    "vrpn_Tracker Pos_Quat" => {
                        self.tracker_pos_quat_message_idx = type_id;
                        trace!("Recognising pos quat message as {type_id}");
                    }
                    _ => {
                        trace!("Unrecognised name '{}'", type_name.as_str())
                    }
                }

                self.remote_type_list.install(
                    type_id as usize,
                    TypeInfo {
                        id: type_id,
                        name: type_name,
                    },
                );

                // bounce all types by default
                write_message(output, type_id, TYPE_CONNTYPE, payload)
            }
            TYPE_UDPDESC => {
                trace!("Skipping UDP description");
                return Ok(());
            }
            TYPE_LOGDESC => {
                trace!("Skipping Log description");
                return Ok(());
            }
            TYPE_DISCONN => {
                // Docs say this will never be sent over the wire...
                trace!("Skipping disconn description");
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

fn transform_position(p: [f64; 3]) -> DVec3 {
    DVec3::new(-p[0], p[2], p[1])
}

fn transform_rotation(p: [f64; 4]) -> DQuat {
    DQuat {
        x: -p[0],
        y: p[2],
        z: p[1],
        w: p[3],
    }
}

fn vrpn_spinner(
    to_watch: HashMap<String, SharedItemState>,
    host_string: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut state = VRPNClient::new(to_watch, &host_string).expect("unable to connect");

    state.service(shutdown);
}

/// We parse names as Item@Host:Port

fn start_vrpn_client(
    to_watch: HashMap<String, SharedItemState>,
    host_string: &str,
    res: &mut VRPNResource,
) {
    let host_string = host_string.to_owned();

    let sd = res.shutdown.clone();

    let handle = std::thread::spawn(|| {
        vrpn_spinner(to_watch, host_string, sd);
    });

    res.vrpn_threads.push(handle);
}

#[derive(Resource)]
pub struct VRPNResource {
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    vrpn_threads: Vec<std::thread::JoinHandle<()>>,
}

impl VRPNResource {
    pub fn wait_for_shutdown(self) {
        self.shutdown
            .store(false, std::sync::atomic::Ordering::Release);
        for t in self.vrpn_threads {
            t.join().unwrap();
        }
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
    reader: SharedItemState,
}

fn check_for_new_vrpn(
    mut commands: Commands,
    query: Query<(Entity, &VRPNLink), Without<VRPNLinkConnected>>,
    mut res: NonSendMut<VRPNResource>,
) {
    //to_watch: HashMap<String, SharedItemState>,

    // split all connections by host.

    // Note that if more of these come along, we do NOT reuse existing connections. thats a WIP.

    let mut map: HashMap<String, HashMap<String, SharedItemState>> = HashMap::default();

    for (e, l) in query.iter() {
        let host = format!("{}:{}", l.host, l.port);

        let state = std::sync::Arc::new(mailbox::Mailbox::new(Default::default()));

        map.entry(host)
            .and_modify(|x| {
                x.insert(l.sender.clone(), state.clone());
            })
            .or_insert_with(|| {
                let mut ret = HashMap::default();
                ret.insert(l.sender.clone(), state.clone());
                ret
            });

        commands
            .entity(e)
            .insert(VRPNLinkConnected { reader: state });
    }

    for (k, v) in map {
        start_vrpn_client(v, &k, &mut res);
    }
}

/// System to service any VRPN content
fn service_vrpn(mut query: Query<(&VRPNLinkConnected, &mut Transform)>) {
    for (c, mut tf) in query.iter_mut() {
        let new_pos = c.reader.read();

        // TODO: rotation. need to check mappings
        tf.translation = new_pos.position.as_vec3();
        tf.rotation = new_pos.rotation.as_quat();
    }
}

pub struct VRPNPlugin;

impl Plugin for VRPNPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VRPNResource {
            shutdown: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            vrpn_threads: vec![],
        });
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
