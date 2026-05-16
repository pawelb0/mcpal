//! Long-running command: open a session and stream every server-initiated
//! notification as one YAML document per event until ctrl-c.

use anyhow::Result;
use mcpal_core::connect;
use tokio::sync::mpsc;

use crate::resolver::resolve;
use crate::runtime::Ctx;

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut handler = ctx.handler.clone();
    handler.events = Some(tx);

    let resolved = resolve(reference, ctx)?;
    let client = connect(&resolved.spec, handler).await?;
    eprintln!("watching {} — press Ctrl-C to exit", resolved.display);

    let mut interrupt = Box::pin(tokio::signal::ctrl_c());
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Some(v) => ctx.render_one(&v)?,
                None => break,
            },
            _ = &mut interrupt => break,
        }
    }

    drop(client);
    Ok(())
}
