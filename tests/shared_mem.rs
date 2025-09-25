use std::env;

use tephrite_rs::multiprocess;
use tephrite_rs::multiprocess::shared_mem::MPCommunicator;

/// Spawns N children that attach, wait at the barrier, verify the payload written by the parent,
/// then each writes a one-byte "ack" at offset 64+idx. Everyone meets at a second barrier.
fn main() {
    // Use the small mapping for tests
    unsafe { env::set_var("TEPHRITE_TEST_PROCESS", "1") };

    if multiprocess::is_child_process() {
        child();
    } else {
        logic();
    }
}

fn logic() {
    let session = multiprocess::generate_session_id();

    crate::multiprocess::install_session_id(&session);

    // Number of children for this test
    const N_CHILDREN: usize = 3;
    let total_processes = (N_CHILDREN as u32) + 1; // +1 for the parent

    // Parent creates the communicator
    let mut parent = MPCommunicator::create(total_processes);

    // Prepare a tiny payload the children will verify
    {
        let buf = parent.data_slice_mut();
        assert!(buf.len() >= 128);
        buf[0..4].copy_from_slice(&[1, 2, 3, 4]);
        // Clear the ACK area [64, 64+N_CHILDREN)
        for i in 0..N_CHILDREN {
            buf[64 + i] = 0;
        }
    }

    // Fork N children
    let current_exe = std::env::current_exe()
        .expect("determine current executable")
        .to_owned();

    let child_list: Vec<_> = (0..N_CHILDREN)
        .map(|i| {
            let current_exe = current_exe.clone();
            let session_clone = multiprocess::session_id();
            std::thread::spawn(move || {
                println!("Spawning {i}...");

                let mut command = std::process::Command::new(current_exe);
                multiprocess::install_ids(&mut command, &session_clone, i as u32);

                let mut command = command.spawn().expect("launching render process");

                let status = command.wait().unwrap();

                println!("Status {i} {status}");
            })
        })
        .collect();

    println!("HERE");

    // Parent hits barrier #1 (payload written already)
    parent.barrier();

    println!("HERE1");

    // Parent hits barrier #2 after children wrote ACKs
    parent.barrier();

    println!("HERE2");

    // Validate ACKs
    {
        let buf = parent.data_slice();
        for i in 0..N_CHILDREN {
            assert_eq!(buf[64 + i], 1, "child {i} did not ACK");
        }
    }

    // Reap children
    for pid in child_list {
        pid.join().unwrap();
    }
}

fn child() {
    // Child process
    let child = MPCommunicator::attach();

    let idx = multiprocess::child_process_id() as usize;

    println!("NEW HERE {idx}");

    // Barrier #1: wait until parent & all children are ready
    child.barrier();

    // Read & verify the payload
    let bytes = child.data_slice();
    assert_eq!(&bytes[0..4], &[1, 2, 3, 4]);

    // Write an ACK byte at a deterministic spot
    let mut child = child; // get &mut for data_slice_mut
    let acks = child.data_slice_mut();
    acks[64 + idx] = 1;

    // Barrier #2: let parent observe all ACKs simultaneously
    child.barrier();
}
