//! Long-running command: open a session and stream every server-initiated
//! notification as one YAML document per event until ctrl-c.

use anyhow::Result;
use mcpal_core::{Handler, connect};
use tokio::sync::mpsc;

use crate::resolver::resolve;
use crate::runtime::Ctx;

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut opts = ctx.handler_opts.clone();
    opts.events = Some(tx);
    let handler = Handler::new(opts);

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
