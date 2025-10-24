use bevy::{app::TerminalCtrlCHandlerPlugin, prelude::*};

use crate::{
    config::get_render_configuration,
    multiprocess::app::make_common_app,
    serialize::{FastRead, transcript_reader::TranscriptReaderResource},
};

/// Function to run a render (or child) process
pub(crate) fn run() -> AppExit {
    let mut app = make_common_app();

    app.add_plugins(TerminalCtrlCHandlerPlugin);

    // Get child config
    let child_config = get_render_configuration();
    let rank = child_config.process_rank;

    info!("{rank}: Running render process {}", std::process::id());

    app.add_plugins(crate::backfill_link::BackfillPlugin);

    // Add in replication components
    app.add_plugins(crate::replication::reader::ReplicationReaderPlugin);

    debug!("{rank}: Render replication ready...");

    // exec
    let result = app.run();

    debug!("{rank}: Stopping renderer...");

    // clean up
    //clean_up(&mut app);

    result
}

fn clean_up(app: &mut App) {
    use crate::replication::instruction::ClientInstruction;

    // wait for a termination

    let mut res = app
        .world_mut()
        .remove_non_send_resource::<TranscriptReaderResource>()
        .unwrap();

    res.consume_next(|_, _, bytes| {
        let mut bytes = crate::serialize::ByteReader::new(bytes);

        loop {
            let instruction = unsafe { ClientInstruction::read_fast(&mut bytes) };

            match instruction {
                ClientInstruction::Halt(_) => {
                    return;
                }
                _ => {}
            }
        }
    });
}
