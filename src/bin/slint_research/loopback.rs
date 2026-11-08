//! The shared synthetic station rejects every input byte and checks Close.
use pttstella::{
    connection::{CloseOutcome, Connection, Phase},
    replay,
    screen::Frame,
};
use std::{error::Error, time::Duration};

pub fn with_grid(
    bytes: Vec<u8>,
    show: impl FnOnce(Frame) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let expected = bytes.len();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let connection = runtime.block_on(async { Connection::start_ui_probe(bytes) })?;
    let result = runtime
        .block_on(receive_grid(&connection, expected))
        .and_then(show);
    let close = runtime.block_on(connection.shutdown());
    match (result, close) {
        (Ok(()), Ok(CloseOutcome::Acknowledged)) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "status": "passed",
                    "fixture": "parity_grid",
                    "received_bytes": expected,
                    "close": CloseOutcome::Acknowledged,
                })
            );
            Ok(())
        }
        (Ok(()), Ok(outcome)) => {
            Err(format!("loopback Close was not acknowledged: {outcome:?}").into())
        }
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(error), Ok(_)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup: {cleanup}").into()),
    }
}

async fn receive_grid(connection: &Connection, expected: usize) -> Result<Frame, Box<dyn Error>> {
    connection.connect()?;
    let mut updates = connection.updates.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            {
                let view = updates.borrow_and_update();
                if view.phase == Phase::Error {
                    return Err(view.message.clone().into());
                }
                if view.received > expected {
                    return Err("loopback returned extra bytes".into());
                }
                if view.phase == Phase::Connected
                    && view.received == expected
                    && !view.output_pending
                {
                    replay::verify_parity_grid(&view.frame)?;
                    return Ok(view.frame.clone());
                }
            }
            updates.changed().await?;
        }
    })
    .await?
}
