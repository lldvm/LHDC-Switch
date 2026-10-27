//! The IPC server: the user's Named Pipe, one task per UI connection.
//!
//! A connection says Hello, gets Welcome and a Snapshot, then the events of
//! every state change; requests run concurrently and their responses go out
//! through the same writer. A client that falls behind on events gets a new
//! snapshot instead (it sees the revision gap).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ipc::{ClientMessage, Event, Method, PROTOCOL_VERSION, ServerMessage, read_frame, write_frame};
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::sync::{broadcast, mpsc};

use crate::state::CoreState;

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// Accepts connections until the process ends. Err if another core of this
/// user owns the pipe.
pub async fn serve(state: Arc<CoreState>, name: String) -> std::io::Result<()> {
    let mut server = win_sys::pipe::create_server(&name, true)?;
    tracing::info!("IPC: listening on {name}");
    loop {
        server.connect().await?;
        let client = server;
        server = win_sys::pipe::create_server(&name, false)?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = session(state, client).await {
                tracing::debug!("IPC: client ended: {e}");
            }
        });
    }
}

async fn session(state: Arc<CoreState>, pipe: NamedPipeServer) -> Result<(), ipc::FrameError> {
    let (mut rd, mut wr) = tokio::io::split(pipe);
    let hello = tokio::time::timeout(HELLO_TIMEOUT, read_frame::<_, ClientMessage>(&mut rd)).await;
    let client = match hello {
        Ok(Ok(Some(ClientMessage::Hello { protocol, client }))) => {
            if protocol != PROTOCOL_VERSION {
                let reason = format!("protocol {protocol} not supported (core speaks {PROTOCOL_VERSION})");
                write_frame(&mut wr, &ServerMessage::Reject { reason }).await?;
                return Ok(());
            }
            client
        }
        _ => {
            let reason = "expected hello".to_string();
            write_frame(&mut wr, &ServerMessage::Reject { reason }).await?;
            return Ok(());
        }
    };
    tracing::info!("IPC: {client} connected");
    write_frame(
        &mut wr,
        &ServerMessage::Welcome {
            protocol: PROTOCOL_VERSION,
            server: format!("a2dpwb-core/{}", env!("CARGO_PKG_VERSION")),
        },
    )
    .await?;
    // Subscribe before the snapshot so no change falls in between
    let mut events = state.subscribe();
    let (rev, snap) = state.snapshot();
    write_frame(&mut wr, &ServerMessage::Snapshot { rev, state: Box::new(snap) }).await?;

    // Responses come from request tasks through this channel
    let (resp_tx, mut resp_rx) = mpsc::channel::<ServerMessage>(32);
    let mut link = false;
    let mut receiver = false;
    // Log lines while subscribed; the sender stays alive for the session so
    // recv never ends
    let (log_tx, mut log_rx) = mpsc::channel::<ipc::LogLine>(1024);
    let mut log_sub: Option<crate::logging::Subscription> = None;
    let result = loop {
        tokio::select! {
            msg = read_frame::<_, ClientMessage>(&mut rd) => {
                match msg {
                    Ok(Some(ClientMessage::Request { id, call })) => {
                        if let Method::DiagSubscribe { logs: l, link: k, receiver: r } = &call {
                            log_sub = l.map(|level| {
                                let tx = log_tx.clone();
                                crate::logging::hub().subscribe_with(level, move |line| {
                                    !matches!(tx.try_send(line.clone()), Err(mpsc::error::TrySendError::Closed(_)))
                                })
                            });
                            set_subscribed(&state.link_subscribers, &mut link, *k);
                            let r = r.unwrap_or(false);
                            if set_subscribed(&state.receiver_subscribers, &mut receiver, r) && r {
                                // The state now, not only at the next tick
                                state.publish_receiver();
                            }
                        }
                        let (state, tx) = (state.clone(), resp_tx.clone());
                        tokio::spawn(async move {
                            let r = state.handle(call).await;
                            let msg = match r {
                                Ok(v) => ServerMessage::Response { id, result: Some(v), error: None },
                                Err(e) => ServerMessage::Response { id, result: None, error: Some(e) },
                            };
                            let _ = tx.send(msg).await;
                        });
                    }
                    Ok(Some(ClientMessage::Hello { .. })) => {}
                    Ok(None) => break Ok(()),
                    Err(e) => break Err(e),
                }
            }
            Some(resp) = resp_rx.recv() => {
                if let Err(e) = write_frame(&mut wr, &resp).await { break Err(e); }
            }
            ev = events.recv() => {
                match ev {
                    Ok((rev, event)) => {
                        let wanted = match &*event {
                            Event::Link(_) => link,
                            Event::Receiver(_) => receiver,
                            Event::Log(_) => false, // logs come from the log hub
                            _ => true,
                        };
                        if wanted {
                            let m = ServerMessage::Event { rev, event: Box::new((*event).clone()) };
                            if let Err(e) = write_frame(&mut wr, &m).await { break Err(e); }
                        }
                        if matches!(&*event, Event::Bye) { break Ok(()); }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let (rev, snap) = state.snapshot();
                        let m = ServerMessage::Snapshot { rev, state: Box::new(snap) };
                        if let Err(e) = write_frame(&mut wr, &m).await { break Err(e); }
                    }
                    Err(broadcast::error::RecvError::Closed) => break Ok(()),
                }
            }
            Some(line) = log_rx.recv() => {
                let (rev, _) = state.snapshot();
                let m = ServerMessage::Event { rev, event: Box::new(Event::Log(line)) };
                if let Err(e) = write_frame(&mut wr, &m).await { break Err(e); }
            }
        }
    };
    drop(log_sub);
    set_subscribed(&state.link_subscribers, &mut link, false);
    set_subscribed(&state.receiver_subscribers, &mut receiver, false);
    tracing::info!("IPC: {client} disconnected");
    result
}

/// Counts this connection in or out of a subscription; true if it changed
fn set_subscribed(count: &AtomicUsize, current: &mut bool, wanted: bool) -> bool {
    if *current == wanted {
        return false;
    }
    *current = wanted;
    if wanted {
        count.fetch_add(1, Ordering::AcqRel);
    } else {
        count.fetch_sub(1, Ordering::AcqRel);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscriptions_count_once_per_connection() {
        let count = AtomicUsize::new(0);
        let (mut a, mut b) = (false, false);
        assert!(set_subscribed(&count, &mut a, true));
        assert!(!set_subscribed(&count, &mut a, true));
        assert!(set_subscribed(&count, &mut b, true));
        assert_eq!(count.load(Ordering::Acquire), 2);
        assert!(set_subscribed(&count, &mut a, false));
        assert!(!set_subscribed(&count, &mut a, false));
        assert_eq!(count.load(Ordering::Acquire), 1);
    }
}
