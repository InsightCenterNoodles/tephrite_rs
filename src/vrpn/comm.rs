//! This module implements a minimal VRPN (Virtual-Reality Peripheral Network)
//! TCP client with just enough of the wire protocol to discover senders and
//! stream tracker poses and inputs.
//!
//! Implementation notes
//! - The VRPN handshake starts by sending a fixed cookie and then reading the
//!   server's cookie to determine protocol version. Only version `7.xx` is
//!   supported here.
//! - VRPN messages are big-endian and aligned to 8-byte boundaries; payloads
//!   are padded to the next multiple of 8 bytes.
//! - For thread-safe cross-thread communication, this module uses a
//!   `seqlock::SeqLock<ItemState>` per watched sender. Writer is the network
//!   thread, readers are Bevy systems.
//! - Coordinate frames differ. The conversion functions reflect the empirical
//!   mapping from a typical VRPN server to Bevy coordinates. Adjust if your
//!   installation uses a different convention.
//!
//! This module is intentionally narrow in scope: it parses only the pieces of
//! VRPN required for tracker position/quaternion updates and ignores other
//! message families.

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

use crate::vrpn::common::SharedItemState;

/// All VRPN messages are aligned to doubles
const VRPN_ALIGN: usize = 8;
/// VRPN magic header byte length
const VRPN_MAGIC_LENGTH: usize = 16;
/// Total size (in bytes) of the initial VRPN cookie sent during handshake.
///
/// The cookie comprises a 16-byte ASCII prefix plus 8 bytes of alignment,
/// which yields 24 bytes overall.
const VRPN_COOKIE_SIZE: usize = VRPN_MAGIC_LENGTH + VRPN_ALIGN;
/// Default port for VRPN connections
const VRPN_DEFAULT_PORT: u16 = 3883;
/// The size of the header in bytes
const HEADER_SIZE_BYTES: usize = size_of::<MessageHeader>();

const _: () = assert!(HEADER_SIZE_BYTES == 24, "Header size does not match spec!");

const SUBTYPE_TRACKER_POS_QUAT: &str = "vrpn_Tracker Pos_Quat";
const SUBTYPE_ANALOG_CHANNEL: &str = "vrpn_Analog Channel";
const SUBTYPE_BUTTON_CHANGE: &str = "vrpn_Button Change";

/// Local alias for IO results used throughout this module.
type Result<T> = std::io::Result<T>;

/// Write our local VRPN cookie to a stream.
///
/// This announces the client's protocol version to the server and starts the
/// handshake. The bytes roughly correspond to: `"vrpn: ver. 07.33  0\0\xed\0\0\0"`.
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

/// Validate a VRPN cookie and extract `(major, minor, log_level)`.
///
/// Returns `None` if the cookie does not start with `"vrpn: ver."` or if the
/// version components are not parseable ASCII digits.
fn check_vrpn_cookie(bytes: &[u8]) -> Option<(u32, u32, u32)> {
    let major = &bytes[11..13];
    let minor = &bytes[14..16];
    let log = bytes[18] as u32;
    const ZERO_ASCII: u32 = 48;

    if bytes[0..10] != *b"vrpn: ver." {
        return None;
    }

    let major: u32 = std::str::from_utf8(major).ok()?.parse().ok()?;
    let minor: u32 = std::str::from_utf8(minor).ok()?.parse().ok()?;
    let log = log - ZERO_ASCII;
    Some((major, minor, log))
}

/// Read a VRPN cookie from a stream into a fixed-size buffer.
fn read_vrpn_cookie(r: &mut impl Read) -> Result<[u8; VRPN_COOKIE_SIZE]> {
    let mut incoming = [0u8; VRPN_COOKIE_SIZE];

    r.read_exact(&mut incoming)?;

    Ok(incoming)
}

/// Read a single big-endian `i32` from the stream.
fn read_be_i32(r: &mut impl Read) -> std::io::Result<i32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(i32::from_be_bytes(b))
}

/// Read a single big-endian IEEE-754 `f64` from the stream.
fn read_be_f64(r: &mut impl Read) -> std::io::Result<f64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(f64::from_bits(u64::from_be_bytes(b)))
}

/// Read `N` big-endian IEEE-754 `f64`s from the stream as an array.
fn read_be_f64_n<const N: usize>(r: &mut impl Read) -> std::io::Result<[f64; N]> {
    let mut out = [0u64; N];
    r.read_exact(cast_slice_mut(&mut out))?;
    Ok(out.map(|x| f64::from_bits(u64::from_be(x))))
}

/// Read `count` big-endian IEEE-754 `f64`s from the stream into an array.
fn read_be_f64_dyn(r: &mut impl Read, count: u64, dest: &mut Vec<f64>) -> std::io::Result<()> {
    dest.resize(count as usize, 0.0);
    r.read_exact(cast_slice_mut(dest.as_mut_slice()))?;

    for x in dest {
        *x = f64::from_bits(u64::from_be(x.to_bits()));
    }

    Ok(())
}

/// Transform position from VRPN coordinates to Bevy coordinates.
///
/// Empirically, this mapping matches common VRPN tracker conventions to
/// Bevy's `X-right, Y-up, Z-forward` coordinates:
/// `[-x, z, y]`.
#[inline]
fn transform_position(p: [f64; 3]) -> DVec3 {
    DVec3::new(-p[0], p[2], p[1])
}

/// Transform rotation from VRPN coordinates to Bevy coordinates.
///
/// The mapping mirrors `transform_position` and flips the X component: `[-x, z, y, w]`.
#[inline]
fn transform_rotation(p: [f64; 4]) -> DQuat {
    DQuat {
        x: -p[0],
        y: p[2],
        z: p[1],
        w: p[3],
    }
}

/// The header of a VRPN message.
///
/// Layout (6 big-endian `i32`s):
/// - `[0] total_len`: Packet length in bytes (header + payload + padding).
/// - `[1] secs`: Timestamp seconds (UNIX epoch).
/// - `[2] micros`: Timestamp microseconds.
/// - `[3] sender`: Sender index (object ID) for this message.
/// - `[4] ty`: Message type. Negative values are system types.
/// - `[5] reserved`: Reserved/unused.
#[derive(Debug, Default)]
struct MessageHeader([i32; 6]);

/// Construct a new header for `sender`, `ty`, and `payload`.
///
/// Fills the timestamp fields from `SystemTime::now()` and computes total
/// packet length including padding to the next multiple of 8.
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

/// Write a payload to a byte sink with a properly formatted VRPN header.
///
/// Ensures the payload is padded to 8-byte alignment as required by VRPN.
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
    /// Read and decode a header from a stream (big-endian fields).
    fn read(reader: &mut impl Read) -> Result<Self> {
        let mut h = MessageHeader::default();
        reader.read_exact(cast_slice_mut(&mut h.0[..]))?;
        h.0 = h.0.map(i32::from_be);
        Ok(h)
    }

    /// Encode and write this header to a stream (big-endian fields).
    fn write(&self, writer: &mut impl Write) -> Result<()> {
        let v = self.0.map(|f| f.to_be());
        writer.write_all(cast_slice(&v))?;

        Ok(())
    }

    /// The length of the entire packet (header + payload + pad bytes).
    #[inline]
    fn packet_length(&self) -> usize {
        self.0[0] as usize
    }

    /// The length of the payload only (excludes header, excludes pad bytes).
    fn payload_length(&self) -> usize {
        self.packet_length() - size_of::<MessageHeader>()
    }

    /// The sender (object) id for this message.
    fn sender(&self) -> i32 {
        self.0[3]
    }

    /// The message type. Negative values are system-reserved types.
    fn ty(&self) -> i32 {
        self.0[4]
    }
}

struct ConnectionInfo {
    senders: HashMap<String, SharedItemState>,
    host: SocketAddr,
}

// =============================================================================

/// Read a single message from `stream`, writing its (padded) payload into
/// `buffer`, and return the decoded header.
///
/// The buffer is reused across calls to avoid repeated allocations.
fn get_message(stream: &mut impl Read, buffer: &mut Vec<u8>) -> Result<MessageHeader> {
    //println!("Getting next message...");

    let header = MessageHeader::read(stream)?;

    //println!("Got header...");

    let payload_length = header.payload_length();

    let ceil_length = payload_length.next_multiple_of(VRPN_ALIGN);

    //println!("Read payload {ceil_length}");

    const MAX_PAYLOAD: usize = 128 * 1024;

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

/// A connection plus protocol state for a specific VRPN server.
pub(crate) struct VRPNClient {
    remote: TcpStream,
    message_state: MessageState,
}

impl VRPNClient {
    /// Create a new connection with a map of items to watch.
    ///
    /// `to_watch` maps VRPN sender names to shared state handles. The client
    /// performs the cookie handshake and validates the server version. On
    /// success, it is ready to `run` and process messages.
    pub(crate) fn new(
        to_watch: HashMap<String, SharedItemState>,
        host_string: &str,
    ) -> Result<Self> {
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

    /// Event loop for the network thread.
    ///
    /// Processes messages until `run` is cleared. Timeouts are expected and
    /// simply cause the loop to continue.
    pub(crate) fn run(&mut self, run: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        let mut buffer = Vec::new();

        let mut rounds = 1u32;
        let mut messages = 0u32;
        let mut skips = 0u32;
        while run.load(std::sync::atomic::Ordering::Acquire) {
            if rounds.is_multiple_of(60) {
                println!("Get message {rounds} {messages} {skips}");
            }
            rounds += 1;
            match get_message(&mut self.remote, &mut buffer) {
                Ok(header) => {
                    if self
                        .message_state
                        .handle_message(header, &buffer, &mut self.remote)
                        .is_err()
                    {
                        messages += 1;
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    skips += 1;
                    continue;
                }
                Err(_) => break,
            }
        }
    }
}

// MARK: Sender List

/// A list of remote senders (objects) and associated state.
///
/// `to_watch` is a name → state map (configured by the app). As the server
/// advertises senders, `install` populates the `watched` map by sender index,
/// which allows fast routing of subsequent updates.
struct SenderList {
    to_watch: HashMap<String, SharedItemState>,
    watched: HashMap<usize, SharedItemState>,
}

/// Defensive limit on known components to avoid unbounded growth from a
/// misbehaving server.
const MAX_KNOWN_COMPONENTS: usize = 1024;

impl SenderList {
    fn new(to_watch: HashMap<String, SharedItemState>) -> Self {
        Self {
            //infos: Vec::with_capacity(128),
            to_watch,
            watched: Default::default(),
        }
    }

    fn is_watching(&self, name: &str) -> bool {
        self.to_watch.contains_key(name)
    }

    #[inline]
    fn lookup(&self, index: usize) -> Result<&SharedItemState> {
        //println!("Lookup index {index}");
        self.watched.get(&index).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Missing component info")
        })
    }

    fn install(&mut self, index: usize, name: String) -> Result<()> {
        if index >= MAX_KNOWN_COMPONENTS {
            // can't install
            warn!("overflow! dropping component");
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Component index overflow",
            ));
        }

        // install

        if let Some(w) = self.to_watch.get(&name) {
            self.watched.insert(index, w.clone());
        }

        Ok(())
    }
}

// MARK: Message State

type Handler = fn(&mut MessageState, usize, &mut Cursor<&[u8]>) -> Result<()>;

/// Per-connection protocol state used to dispatch and decode messages.
struct MessageState {
    remote_sender_list: SenderList,

    message_type_handlers: Vec<Option<Handler>>,
}

impl MessageState {
    fn new(to_watch: HashMap<String, SharedItemState>) -> Self {
        let remote_sender_list = SenderList::new(to_watch);

        Self {
            remote_sender_list,
            message_type_handlers: Vec::with_capacity(64),
        }
    }

    fn alloc_handler(&mut self, index: i32, func: Handler) {
        let Ok(index) = index.try_into() else {
            return;
        };

        if self.message_type_handlers.len() <= index {
            self.message_type_handlers.resize(index + 10, None);
        }

        self.message_type_handlers[index] = Some(func);
    }

    /// Decode a `vrpn_Tracker Pos_Quat` message body and update shared state.
    fn handle_pos_quat(&mut self, sender: usize, source: &mut Cursor<&[u8]>) -> Result<()> {
        // First double is a timestamp (seconds since server start). Skip it.
        let _dummy = read_be_f64(source)?;

        let pos: [f64; 3] = read_be_f64_n(source)?;
        let quat: [f64; 4] = read_be_f64_n(source)?;

        //dbg!(pos, quat);

        if let Ok(item) = self.remote_sender_list.lookup(sender) {
            let mut lock = item.lock().unwrap();
            lock.position = transform_position(pos);
            lock.rotation = transform_rotation(quat);
        }

        Ok(())
    }

    fn handle_analog(&mut self, sender: usize, source: &mut Cursor<&[u8]>) -> Result<()> {
        //debug!("Got analog update");
        // First is a 64 bit float(!!) which is the num of channels.
        // This is in the spec, dont blame me
        let channel_count = read_be_f64(source)?;

        let channel_count: u64 = channel_count.floor() as u64;

        // 128 is listed as the channel max in the source
        if channel_count > 128 {
            return Err(std::io::Error::other("Bad analog channel count"));
        }

        // read those into a vector

        if let Ok(item) = self.remote_sender_list.lookup(sender) {
            let mut lock = item.lock().unwrap();

            read_be_f64_dyn(source, channel_count, &mut lock.analog_state)?;
        }

        Ok(())
    }

    fn handle_button_change(&mut self, sender: usize, source: &mut Cursor<&[u8]>) -> Result<()> {
        //debug!("Got button update");
        // Should be a pair of i32, button, then state
        let button: u8 = read_be_i32(source)?
            .try_into()
            .map_err(|_| std::io::Error::other("invalid button index"))?;
        let state: u8 = read_be_i32(source)?
            .try_into()
            .map_err(|_| std::io::Error::other("invalid button state"))?;

        if let Ok(item) = self.remote_sender_list.lookup(sender) {
            let mut lock = item.lock().unwrap();

            // by the source, the max button index fits in a u8

            if lock.button_changes.len() > 256 {
                // uh oh, we dont want to overflow here.
                warn!("Button change queue overflow!");
                lock.button_changes.pop_front();
            }

            lock.button_changes.push_back((button, state));
        }

        Ok(())
    }

    // fn handle_button_change(&mut self, sender: usize, source: &mut impl Read) -> Result<()> {
    //     // Should be a pair of i32, button, then state
    //     let button = read_be_i32(source)?;
    //     let state = read_be_i32(source)?;

    //     if let Ok(item) = self.remote_sender_list.lookup(sender) {
    //         let mut lock = item.write().unwrap();

    //         if let Some(x) = lock.button_state.get_mut(button as usize) {
    //             *x = state as u8;
    //         }
    //     }

    //     Ok(())
    // }

    /// Dispatch a single message using the decoded `header` and `payload`.
    ///
    /// Some system messages are bounced back to the server (e.g. type and
    /// sender announcements) as part of the VRPN protocol handshake.
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

            const MAX_NAME: i32 = 64 * 1024;

            if len <= 0 || len > MAX_NAME {
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
            v if v >= 0 => {
                if let Some(Some(x)) = self.message_type_handlers.get(v as usize) {
                    x(self, header.sender() as usize, &mut cursor)
                } else {
                    Ok(())
                }
            }
            TYPE_SENDER => {
                let sender_id = header.sender();
                let sender_name = extract_prefixed_string(&mut cursor)?;
                debug!("New sender {sender_id}: '{sender_name}'");

                let bounce = sender_name.starts_with("VRPN Control")
                    || self.remote_sender_list.is_watching(&sender_name);

                self.remote_sender_list
                    .install(sender_id as usize, sender_name.clone())?;

                if bounce {
                    return write_message(output, sender_id, TYPE_SENDER, payload);
                }

                Ok(())
            }
            TYPE_CONNTYPE => {
                let type_id = header.sender();
                let type_name = extract_prefixed_string(&mut cursor)?;
                debug!("New type {type_id}: '{type_name}'");

                match type_name.as_str() {
                    SUBTYPE_TRACKER_POS_QUAT => {
                        self.alloc_handler(type_id, Self::handle_pos_quat);
                    }
                    SUBTYPE_ANALOG_CHANNEL => {
                        self.alloc_handler(type_id, Self::handle_analog);
                    }
                    SUBTYPE_BUTTON_CHANGE => {
                        self.alloc_handler(type_id, Self::handle_button_change);
                    }
                    _ => {
                        trace!("Unrecognised name '{}'", type_name.as_str())
                    }
                }

                // bounce all types by default
                write_message(output, type_id, TYPE_CONNTYPE, payload)
            }
            TYPE_UDPDESC => {
                trace!("Skipping UDP description");
                Ok(())
            }
            TYPE_LOGDESC => {
                trace!("Skipping Log description");
                Ok(())
            }
            TYPE_DISCONN => {
                // Docs say this will never be sent over the wire...
                trace!("Skipping disconn description");
                Ok(())
            }
            _ => {
                //println!("Unknown message type: {x}");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use bevy::math::{DVec4, dvec3, dvec4};

    use crate::vrpn::common::new_shared_item_state;

    use super::{MessageState, check_vrpn_cookie, get_message};

    #[test]
    fn basic_decode() {
        let mut bytes: &[u8] = &[0x40, 0x25, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00];

        assert_eq!(10.75, super::read_be_f64(&mut bytes).unwrap());

        let mut data: &[u8] = &[
            0x40, 0x24, 0xe6, 0x66, 0x66, 0x66, 0x66, 0x66, //
            0x40, 0xb9, 0x34, 0x4b, 0x1c, 0x43, 0x2c, 0xa5, //
            0x40, 0x3c, 0x05, 0x1e, 0xb8, 0x51, 0xeb, 0x85, //
            0xc0, 0x94, 0x34, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let arr: [_; 4] = super::read_be_f64_n(&mut data).unwrap();

        let truth = [10.45, 6452.2934, 28.02, -1293.0];

        assert_eq!(arr, truth);

        let mut vec = vec![];

        let mut data: &[u8] = &[
            0x40, 0x24, 0xe6, 0x66, 0x66, 0x66, 0x66, 0x66, //
            0x40, 0xb9, 0x34, 0x4b, 0x1c, 0x43, 0x2c, 0xa5, //
            0x40, 0x3c, 0x05, 0x1e, 0xb8, 0x51, 0xeb, 0x85, //
            0xc0, 0x94, 0x34, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        super::read_be_f64_dyn(&mut data, 4, &mut vec).unwrap();

        itertools::assert_equal(truth.iter(), vec.iter());
    }

    #[test]
    fn check_cookie() {
        const COOKIE: &[u8; 24] = include_bytes!("../../test_assets/vrpn/0_remote_cookie.bin");

        assert_eq!(check_vrpn_cookie(COOKIE).unwrap(), (7, 33, 0));
    }

    fn handle_bytes<F>(path: &str, mut f: F)
    where
        F: FnMut(&mut Cursor<Vec<u8>>),
    {
        let data = std::fs::read(path).unwrap();
        let to_read = data.len();
        let mut src_cursor = Cursor::new(data);

        while src_cursor.position() as usize != to_read {
            f(&mut src_cursor);
        }
    }

    #[test]
    fn test_read() {
        let data = std::fs::read("test_assets/vrpn/1_content.bin").unwrap();
        let to_read = data.len();
        let mut src_cursor = Cursor::new(data);

        let head_state = new_shared_item_state();
        let joy_state = new_shared_item_state();

        let mut buffer = Vec::new();
        let mut state = MessageState::new(
            [
                ("Head0".into(), head_state.clone()),
                ("Joystick0".into(), joy_state.clone()),
            ]
            .into(),
        );

        let mut output = Vec::new();

        while src_cursor.position() as usize != to_read {
            let header = get_message(&mut src_cursor, &mut buffer).unwrap();

            state.handle_message(header, &buffer, &mut output).unwrap();
        }

        handle_bytes("test_assets/vrpn/2_content.bin", |c| {
            let header = get_message(c, &mut buffer).unwrap();

            state.handle_message(header, &buffer, &mut output).unwrap();
        });

        output.clear();

        assert!(
            head_state.lock().unwrap().position.distance(dvec3(
                0.12531011353529492,
                0.8024732135411116,
                0.867021419799275,
            )) < 0.0001
        );

        let head_rot: DVec4 = head_state.lock().unwrap().rotation.into();

        //dbg!(head_rot);

        assert!(
            head_rot.distance(dvec4(
                -0.020877551525891363,
                0.47324795399249914,
                0.01506643379176776,
                0.8805529538062977,
            )) < 0.0001
        );

        assert!(
            joy_state.lock().unwrap().position.distance(dvec3(
                -0.4458300779842322,
                0.8178812792378916,
                2.1620918226860906,
            )) < 0.0001
        );

        let joy_rot: DVec4 = joy_state.lock().unwrap().rotation.into();

        //dbg!(joy_rot);

        assert!(
            joy_rot.distance(dvec4(
                -0.016291261755337204,
                0.5886627161145235,
                0.009137776752529683,
                0.8081629182801645,
            )) < 0.0001
        );
    }
}
