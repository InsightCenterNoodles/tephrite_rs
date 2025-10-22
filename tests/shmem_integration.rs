//! Integration tests for SharedMemory (no fork).
use std::hash::{Hash, Hasher};
use std::io::Result;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use libc::getpid;

// Adjust this path to your crate root:
use tephrite_rs::multiprocess::shared_mem::SharedMemory;

/// Build a unique shm name per run to avoid collisions between parallel CI jobs.
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

/// Minimal helper to print and propagate errors.
fn run(name: &str, f: impl FnOnce() -> Result<()>) {
    match f() {
        Ok(()) => eprintln!("[ok]   {name}"),
        Err(e) => {
            eprintln!("[FAIL] {name}: {e:?}");
            std::process::exit(1);
        }
    }
}

// MARK: Regular single-process tests

fn test_create_and_basic_rw() -> Result<()> {
    let key = unique_key("basic");
    let size = 4096_usize;

    let mut owner = SharedMemory::create(&key, size)?;
    // memory should be mapped and mutable
    let buf = owner.as_slice_mut();

    // We might allocate more than requested to align to pages
    assert!(buf.len() >= size as usize);

    // Write a pattern and read it back via the same mapping
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let view = owner.as_slice();
    for (i, b) in view.iter().enumerate() {
        assert_eq!(*b, (i % 251) as u8);
    }
    Ok(())
}

fn test_attach_sees_writes() -> Result<()> {
    let key = unique_key("attach");
    let size = 8192_usize;

    // create and write
    let mut owner = SharedMemory::create(&key, size)?;
    let owner_slice = owner.as_slice_mut();
    owner_slice.fill(0);
    owner_slice[0..4].copy_from_slice(&[1, 2, 3, 4]);

    // second mapping in the same process acts like another "process"
    let att = SharedMemory::attach(&key)?;
    let att_slice = att.as_slice();
    assert_eq!(&att_slice[0..4], &[1, 2, 3, 4]);

    // mutate from attached; see from owner
    drop(att); // keep attached alive first to test below as well
    let att2 = SharedMemory::attach(&key)?;
    {
        // write via att2
        let mut_att2 = unsafe {
            // safe because the SharedMemory invariant guarantees a valid mapping of size bytes.
            std::slice::from_raw_parts_mut(att2.as_slice().as_ptr() as *mut u8, size as usize)
        };
        mut_att2[100..104].copy_from_slice(&[9, 8, 7, 6]);
    }
    assert_eq!(&owner.as_slice()[100..104], &[9, 8, 7, 6]);

    // Clean up
    drop(att2);
    drop(owner);
    Ok(())
}

fn test_unlink_on_drop_blocks_new_attach_but_existing_mapping_works() -> Result<()> {
    let key = unique_key("unlink");
    let size = 4096;

    // Create and take a second mapping.
    let mut owner = SharedMemory::create(&key, size)?;
    let att = SharedMemory::attach(&key)?;
    // Write something observable
    owner.as_slice_mut()[0] = 42;

    // Drop owner: should shm_unlink(name). Existing mapping (att) stays valid.
    drop(owner);

    // Existing mapping still readable.
    assert_eq!(att.as_slice()[0], 42);

    // But a *new* attach should fail because the name is unlinked.
    let res = SharedMemory::attach(&key);
    assert!(res.is_err(), "expected attach after unlink to fail");

    // Drop the attached mapping last (object should be fully gone afterwards).
    drop(att);

    // A second attempt must still fail (name is gone).
    let res2 = SharedMemory::attach(&key);
    assert!(res2.is_err());

    Ok(())
}

fn test_open_nonexistent_fails() -> Result<()> {
    // Use a name that should not exist.
    let key = unique_key("noexist");
    let res = SharedMemory::attach(&key);
    assert!(res.is_err(), "attach to non-existent key should error");
    Ok(())
}

fn test_large_size_guard() -> Result<()> {
    // Create a reasonably large region to exercise conversion paths.
    // (We avoid truly huge mappings to keep CI stable.)
    let key = unique_key("largesize");
    let size = 2 * 1024 * 1024; // 2 MiB
    let shm = SharedMemory::create(&key, size)?;
    assert_eq!(shm.as_slice().len(), size as usize);
    Ok(())
}

/// Sanity: multiple attaches in the same process point to the same underlying object.
fn test_multiple_attaches_consistency() -> Result<()> {
    let key = unique_key("multiatt");
    let size = 16 * 1024;
    let mut owner = SharedMemory::create(&key, size)?;

    let a1 = SharedMemory::attach(&key)?;
    let mut a2 = SharedMemory::attach(&key)?;

    owner.as_slice_mut()[7] = 0xAB;
    assert_eq!(a1.as_slice()[7], 0xAB);
    a2.as_slice_mut()[8] = 0xCD;
    assert_eq!(owner.as_slice()[8], 0xCD);

    drop(a1);
    drop(a2);
    drop(owner);
    Ok(())
}

// MARK: Child-mode entry points (spawned)

/// Child mode: attach and write a known pattern at offsets, then exit 0 if ok.
fn child_attach_write(key: &str, offset: usize, bytes: &[u8]) -> Result<()> {
    let mut sh = SharedMemory::attach(key)?;
    let s = sh.as_slice_mut();
    s[offset..offset + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

/// Child mode: attach and wait until control flag becomes `1`, then verify payload and exit.
/// Layout:
/// [0] u8 control_flag (0=wait, 1=go)
/// [1..=4] payload [1,2,3,4]
fn child_wait_and_verify_after_unlink(key: &str) -> Result<()> {
    eprintln!("child_wait_and_verify_after_unlink {key}");
    let mut sh = SharedMemory::attach(key)?;
    {
        // Signal READY to the parent
        let s = sh.as_slice_mut();
        s[0] = 0x7F;
    }

    // Now wait for parent to set GO (1), *after* it unlinks.
    let view = sh.as_slice();
    for _ in 0..20_000 {
        if view[0] == 1 {
            break;
        }
        std::thread::sleep(Duration::from_micros(100));
    }

    // After parent unlinks, our existing mapping must still be valid; verify payload
    assert_eq!(&view[1..5], &[1, 2, 3, 4]);
    Ok(())
}

// MARK: Cross-process tests

fn test_cross_process_write_visible() -> Result<()> {
    let key = unique_key("xproc_write");
    let size = 4096_usize;

    let mut owner = SharedMemory::create(&key, size)?;
    owner.as_slice_mut().fill(0);

    // Spawn child to write a pattern at some offset
    let exe = std::env::current_exe().expect("current_exe");
    let status = Command::new(exe)
        .arg("--child")
        .arg("attach_write")
        .arg(&key)
        .arg("100") // offset
        .arg("DEADBEEF") // hex payload we pass down
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("spawn child attach_write");

    assert!(status.success(), "child process failed");

    // Parent sees child's writes
    let got = &owner.as_slice()[100..104];
    assert_eq!(got, &[0xDE, 0xAD, 0xBE, 0xEF]);

    Ok(())
}

fn test_child_keeps_mapping_after_owner_unlink() -> Result<()> {
    let key = unique_key("xproc_unlink_survive");
    let size = 4096_usize;

    // Parent creates region, initializes control area and payload
    let mut owner = SharedMemory::create(&key, size)?;
    {
        let s = owner.as_slice_mut();
        s[0] = 0; // control flag = WAIT
        s[1..5].copy_from_slice(&[1, 2, 3, 4]); // payload
    }

    // Spawn child that will set READY then later verify after unlink
    let exe = std::env::current_exe().expect("current_exe");
    let mut child = std::process::Command::new(exe)
        .arg("--child")
        .arg("wait_verify_after_unlink")
        .arg(&key)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("spawn child wait_verify_after_unlink");

    // Wait until child signals READY (0x7F)
    {
        use std::time::Duration;
        let s = owner.as_slice();
        let mut waited = 0u32;
        while s[0] != 0x7F {
            std::thread::sleep(Duration::from_millis(1));
            waited += 1;
            if waited > 5_000 {
                panic!("child did not signal READY in time");
            }
        }
    }

    // Tell child to GO, then drop owner to unlink name
    owner.as_slice_mut()[0] = 1; // GO
    drop(owner); // shm_unlink(name); child's existing mapping remains valid

    // Wait for child; it should succeed if mapping survived
    let status = child.wait().expect("wait child");
    assert!(status.success(), "child did not survive/read after unlink");

    // New attach should fail because name is gone
    let res = SharedMemory::attach(&key);
    assert!(res.is_err(), "attach should fail after unlink");

    Ok(())
}

// Main

fn main() {
    let mut args = std::env::args();
    let _exe = args.next();
    if let Some(flag) = args.next() {
        if flag == "--child" {
            let mode = args.next().expect("child mode missing");
            match mode.as_str() {
                "attach_write" => {
                    let key = args.next().expect("key missing");
                    let offset: usize = args
                        .next()
                        .expect("offset missing")
                        .parse()
                        .expect("bad offset");
                    let hex = args.next().expect("hex payload missing");
                    let bytes = hex_string_to_bytes(&hex).expect("bad hex");
                    if let Err(e) = child_attach_write(&key, offset, &bytes) {
                        eprintln!("child attach_write error: {e:?}");
                        std::process::exit(2);
                    }
                    std::process::exit(0);
                }
                "wait_verify_after_unlink" => {
                    let key = args.next().expect("key missing");
                    if let Err(e) = child_wait_and_verify_after_unlink(&key) {
                        eprintln!("child wait_verify_after_unlink error: {e:?}");
                        std::process::exit(3);
                    }
                    std::process::exit(0);
                }
                other => {
                    eprintln!("unknown child mode: {other}");
                    std::process::exit(10);
                }
            }
        }
    }

    // Normal parent-mode test run
    run("create_and_basic_rw", test_create_and_basic_rw);
    run("attach_sees_writes", test_attach_sees_writes);
    run(
        "unlink_blocks_new_attach_but_existing_mapping_works",
        test_unlink_on_drop_blocks_new_attach_but_existing_mapping_works,
    );
    run("open_nonexistent_fails", test_open_nonexistent_fails);
    run("large_size_guard", test_large_size_guard);
    run(
        "multiple_attaches_consistency",
        test_multiple_attaches_consistency,
    );

    // New multi-process tests
    run(
        "cross_process_write_visible",
        test_cross_process_write_visible,
    );
    run(
        "child_keeps_mapping_after_owner_unlink",
        test_child_keeps_mapping_after_owner_unlink,
    );

    eprintln!("all shmem integration tests passed");
}

// MARK: Utilities

fn hex_string_to_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = from_hex_digit(bytes[i])?;
        let lo = from_hex_digit(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn from_hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
