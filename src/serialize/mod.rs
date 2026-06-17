//! Serialization utilities used by the multiprocess pipeline.
//!
//! This module provides a lightweight, allocation‑free binary format intended
//! for high‑throughput data exchange between processes/threads. It is built on:
//!
//! - `fast_io`: minimal byte Readers/Writers (`ByteSource`/`ByteSink`) that
//!   operate on in‑memory slices with bounds checks and panic on overflow.
//! - `fast_ser`: `FastWrite`/`FastRead` traits and helpers for encoding common
//!   Rust/Bevy types. Many implementations are `unsafe` for performance, so
//!   callers must respect invariants documented on each trait.
//! - Type adapters for common Bevy assets and math types.
//! - Mesh/image/material serializers tuned for the replication pipeline.
//! - Transcript reader/writer that integrate with the shared memory ring buffer
//!   in `multiprocess`.
//!
//! Design notes
//! - Endianness is native: both writer and reader must be the same
//!   architecture. This is acceptable for local multi‑process usage.
//! - Many encoders write a length prefix followed by raw bytes. For POD data
//!   the representation is a direct memcpy of the in‑memory layout.
//! - Out‑of‑bounds conditions panic. Keep buffers sized appropriately for the
//!   expected payloads.
//! - The format is not stable across crate versions; it is an internal detail
//!   optimized for speed rather than long‑term compatibility.
pub(crate) mod asset;
pub(crate) mod components;
pub(crate) mod fast_io;
pub(crate) mod fast_ser;
pub(crate) mod image;
pub(crate) mod material;
pub(crate) mod math;
pub(crate) mod mesh;
mod resources;
pub(crate) mod transcript_reader;
pub(crate) mod transcript_writer;

pub use asset::RemappableAsset;
pub use fast_io::*;
pub use fast_ser::*;
