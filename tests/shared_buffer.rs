// tests/lockstep_smoke.rs
#![allow(clippy::needless_return)]

// tests/common.rs
use std::time::{SystemTime, UNIX_EPOCH};

use std::hash::{Hash, Hasher};

use libc::getpid;

fn unique_key(suffix: &str) -> String {
    let pid = unsafe { getpid() };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // On macOS the name length limit is small; hash down to a short name.
    let mut h = std::hash::DefaultHasher::new();
    suffix.hash(&mut h);
    pid.hash(&mut h);
    ts.hash(&mut h);
    let k = h.finish();

    format!("/s_{k}")
}

#[derive(Clone, Copy, Debug)]
pub struct TestParams {
    pub num_buffers: usize,
    pub buf_size: usize,
    pub num_consumers: usize,
    pub frames: u64,
}
impl Default for TestParams {
    fn default() -> Self {
        Self {
            num_buffers: 3,
            buf_size: 4096,
            num_consumers: 3,
            frames: 200,
        }
    }
}

pub fn write_frame_pattern(ptr: *mut u8, buf_size: usize, gen_id: u64, slot: u32) {
    // Simple, deterministic content: [u64 gen][u32 slot][u32 checksum] + payload pattern
    // Safety: caller provides valid, exclusively-writable region
    unsafe {
        let p = ptr as *mut u8;
        *(p as *mut u64) = gen_id;
        *(p.add(8) as *mut u32) = slot;
        // payload: fill with (gen ^ slot) as byte
        let byte = ((gen_id ^ slot as u64) & 0xFF) as u8;
        for i in 16..buf_size {
            *p.add(i) = byte;
        }
        // very lightweight checksum over first 64 bytes of payload
        let mut c: u32 = 0;
        for i in 16..80.min(buf_size) {
            c = c.wrapping_add(*p.add(i) as u32).rotate_left(5);
        }
        *(p.add(12) as *mut u32) = c;
    }
}

pub fn verify_frame_pattern(slice: &[u8], buf_size: usize, expect_gen: u64, slot: u32) {
    unsafe {
        let p = slice.as_ptr();
        let gen_id = *(p as *const u64);
        assert_eq!(gen_id, expect_gen, "frame gen mismatch");

        let s = *(p.add(8) as *const u32);
        assert_eq!(s, slot, "slot meta mismatch");

        let byte = ((expect_gen ^ slot as u64) & 0xFF) as u8;

        // Quick spot checks
        for &i in &[16usize, 31, 79] {
            if i < buf_size {
                assert_eq!(*p.add(i), byte, "payload pattern mismatch @ {}", i);
            }
        }

        // Recompute tiny checksum
        let mut c: u32 = 0;
        for i in 16..80.min(buf_size) {
            c = c.wrapping_add(*p.add(i) as u32).rotate_left(5);
        }
        let c_stored = *(p.add(12) as *const u32);
        assert_eq!(c, c_stored, "checksum mismatch");
    }
}

// MARK: Unix
#[cfg(unix)]
mod unix_only {
    use std::process::{Command, Stdio};
    use std::str::FromStr;
    use std::time::Duration;

    use super::*;

    use tephrite_rs::multiprocess::shared_buffer::{
        Consumer, Producer, compute_shmem_allocation_size,
    };

    // Parent orchestrator: spawns the same binary in "roles".
    // We rely on Cargo setting TEST_BINARY environment for this file.
    fn this_test_path() -> std::path::PathBuf {
        // Cargo exposes current binary path via argv[0]
        std::env::current_exe().expect("current_exe")
    }

    fn run_role(args: &[(&str, String)]) -> std::process::Child {
        let exe = this_test_path();
        let mut cmd = Command::new(exe);
        cmd.arg("--role=child");
        for (k, v) in args {
            cmd.arg(format!("--{}={}", k, v));
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        cmd.spawn().expect("spawn child")
    }

    fn parse_arg<T: FromStr>(name: &str) -> T
    where
        <T as FromStr>::Err: std::fmt::Debug,
    {
        let s = std::env::args()
            .find(|a| a.starts_with(&format!("--{}=", name)))
            .unwrap_or_else(|| panic!("missing arg --{}", name));
        let (_, v) = s.split_once('=').unwrap();
        v.parse::<T>().unwrap()
    }

    // MARK: Child roles

    fn child_producer(key: &str, p: TestParams) {
        let total = compute_shmem_allocation_size(p.num_buffers, p.buf_size);
        println!("[producer] total_size = {}", total);

        let mut prod = Producer::new(key, p.num_buffers, p.buf_size, p.num_consumers).unwrap();

        for _ in 0..p.frames {
            prod.publish_frame_strict(|gen_id, slot, buf| {
                // Write header + payload using your helper
                write_frame_pattern(buf.as_mut_ptr(), buf.len(), gen_id, slot);
            });
        }
        println!("[producer] done");
    }

    fn child_consumer(key: &str, id: usize, expect_frames: u64, buf_size: usize) {
        println!("[consumer#{}] expect frames {}", id, expect_frames);
        let mut cons = Consumer::new(key, id).unwrap();
        let mut seen = 0u64;

        while seen < expect_frames - 1 {
            cons.consume_next(|gen_id, slot, slice| {
                // validate content
                verify_frame_pattern(slice, buf_size, gen_id, slot);
            });
            seen += 1;
        }
        println!("[consumer#{}] done after {}", id, seen);
    }

    // MARK: Main entry: orchestrator or child

    fn child_main() {
        let role = parse_arg::<String>("actor");
        let key = parse_arg::<String>("key");

        match role.as_str() {
            "producer" => {
                let num_buffers = parse_arg::<usize>("num_buffers");
                let buf_size = parse_arg::<usize>("buf_size");
                let num_consumers = parse_arg::<usize>("num_consumers");
                let frames = parse_arg::<u64>("frames");
                let p = TestParams {
                    num_buffers,
                    buf_size,
                    num_consumers,
                    frames,
                };
                child_producer(&key, p);
            }
            "consumer" => {
                let id = parse_arg::<usize>("id");
                let frames = parse_arg::<u64>("frames");
                let buf_size = parse_arg::<usize>("buf_size");
                child_consumer(&key, id, frames, buf_size);
            }
            _ => panic!("unknown child actor"),
        }
    }

    fn orchestrator_main() {
        // Parameters for the run
        let p = TestParams::default();
        let key = unique_key("sxlock_smoke");

        // Now producer
        let mut producer = run_role(&[
            ("actor", "producer".into()),
            ("key", key.clone()),
            ("num_buffers", p.num_buffers.to_string()),
            ("buf_size", p.buf_size.to_string()),
            ("num_consumers", p.num_consumers.to_string()),
            ("frames", p.frames.to_string()),
        ]);

        std::thread::sleep(Duration::from_secs(1));

        let children: Vec<_> = (0..p.num_consumers)
            .map(|id| {
                run_role(&[
                    ("actor", "consumer".into()),
                    ("key", key.clone()),
                    ("id", id.to_string()),
                    ("frames", p.frames.to_string()),
                    ("buf_size", p.buf_size.to_string()),
                ])
            })
            .collect();

        for mut id in children {
            let result = id.wait().unwrap();
            assert!(result.success(), "consumer {} failed", result);
        }

        assert!(producer.wait().unwrap().success(), "producer failed");
    }

    pub fn main() {
        if std::env::args().any(|a| a == "--role=child") {
            child_main();
        } else {
            orchestrator_main();
        }
    }
}

#[cfg(unix)]
fn main() {
    unix_only::main()
}

#[cfg(not(unix))]
fn main() {
    println!("These tests currently run only on Unix (shared mem backend).");
    std::process::exit(0);
}
