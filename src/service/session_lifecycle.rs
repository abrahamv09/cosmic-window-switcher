// SPDX-License-Identifier: GPL-3.0-only

use std::{sync::mpsc, thread};

use anyhow::{Context, Result, bail};
use cosmic_window_switcher::{
    ServiceEvent, SessionInterruption, SessionLifecycleModel, SessionLifecycleSignal,
};
use zbus::{
    MatchRule,
    blocking::{Connection, MessageIterator, Proxy},
    message::{Message, Type},
    zvariant::OwnedObjectPath,
};

use super::PendingLifecycleEvents;

const LOGIN_DESTINATION: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER_INTERFACE: &str = "org.freedesktop.login1.Manager";
const SESSION_INTERFACE: &str = "org.freedesktop.login1.Session";
const USER_INTERFACE: &str = "org.freedesktop.login1.User";

pub(super) struct SessionLifecycleMonitor {
    _thread: thread::JoinHandle<()>,
}

pub(super) fn monitor(events: PendingLifecycleEvents) -> Result<SessionLifecycleMonitor> {
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let thread = thread::Builder::new()
        .name("session-lifecycle".to_owned())
        .spawn(move || {
            let result = listen(&events, &ready_sender);
            if let Err(error) = result {
                let startup_error = format!("{error:#}");
                if ready_sender.send(Err(error)).is_err() {
                    events.push(ServiceEvent::SessionInterrupted(
                        SessionInterruption::ScreenLock,
                    ));
                    eprintln!(
                        "COSMIC Session lifecycle observation stopped; invocations are disabled: {startup_error}"
                    );
                }
            }
        })
        .context("start the COSMIC Session lifecycle observer")?;
    ready_receiver
        .recv()
        .context("the COSMIC Session lifecycle observer stopped during startup")??;
    Ok(SessionLifecycleMonitor { _thread: thread })
}

fn listen(events: &PendingLifecycleEvents, ready: &mpsc::SyncSender<Result<()>>) -> Result<()> {
    let connection = Connection::system().context("connect to system login manager")?;
    let manager = Proxy::new(
        &connection,
        LOGIN_DESTINATION,
        MANAGER_PATH,
        MANAGER_INTERFACE,
    )
    .context("create login manager proxy")?;
    let session_path = resolve_session_path(&connection, &manager)?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(LOGIN_DESTINATION)?
        .build();
    let messages = MessageIterator::for_match_rule(rule, &connection, Some(32))
        .context("subscribe to login-session lifecycle signals")?;
    let session = session_proxy(&connection, &session_path)?;
    let active = session
        .get_property::<bool>("Active")
        .context("read initial login-session activity")?;
    let locked = session
        .get_property::<bool>("LockedHint")
        .context("read initial login-session lock state")?;
    let mut lifecycle = SessionLifecycleModel::new();
    publish(
        events,
        lifecycle.handle(SessionLifecycleSignal::Active(active)),
    );
    publish(
        events,
        lifecycle.handle(SessionLifecycleSignal::Locked(locked)),
    );
    ready
        .send(Ok(()))
        .context("publish lifecycle observer readiness")?;

    for message in messages {
        handle_message(
            events,
            &connection,
            &session_path,
            &mut lifecycle,
            &message.context("receive a login-session lifecycle signal")?,
        )?;
    }
    bail!("the system login manager signal stream ended")
}

fn resolve_session_path(connection: &Connection, manager: &Proxy<'_>) -> Result<OwnedObjectPath> {
    if let Ok(session_path) =
        manager.call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
    {
        return Ok(session_path);
    }

    // A user service belongs to user@.service rather than session-N.scope, so
    // resolve its user's authoritative graphical display session instead.
    let user_path: OwnedObjectPath = manager
        .call("GetUserByPID", &(std::process::id(),))
        .context("resolve the Switcher Service login user")?;
    let user = Proxy::new(
        connection,
        LOGIN_DESTINATION,
        user_path.as_str(),
        USER_INTERFACE,
    )
    .context("create login-user proxy")?;
    let (_session_id, session_path): (String, OwnedObjectPath) = user
        .get_property("Display")
        .context("resolve the user's graphical display session")?;
    if session_path.as_str() == "/" {
        bail!("the login user has no graphical display session");
    }
    Ok(session_path)
}

fn handle_message(
    events: &PendingLifecycleEvents,
    connection: &Connection,
    session_path: &OwnedObjectPath,
    lifecycle: &mut SessionLifecycleModel,
    message: &Message,
) -> Result<()> {
    let header = message.header();
    let member = header.member().map(zbus::names::MemberName::as_str);
    let path = header.path().map(zbus::zvariant::ObjectPath::as_str);
    let signal = match (path, member) {
        (Some(MANAGER_PATH), Some("PrepareForSleep")) => Some(
            SessionLifecycleSignal::PreparingForSleep(message.body().deserialize()?),
        ),
        (Some(MANAGER_PATH), Some("PrepareForShutdown")) => Some(
            SessionLifecycleSignal::PreparingForShutdown(message.body().deserialize()?),
        ),
        (Some(MANAGER_PATH), Some("SessionRemoved")) => {
            let (_id, removed_path): (String, OwnedObjectPath) = message.body().deserialize()?;
            (removed_path == *session_path)
                .then_some(SessionLifecycleSignal::PreparingForShutdown(true))
        }
        (Some(path), Some("Lock")) if path == session_path.as_str() => {
            Some(SessionLifecycleSignal::Locked(true))
        }
        (Some(path), Some("Unlock")) if path == session_path.as_str() => {
            Some(SessionLifecycleSignal::Locked(false))
        }
        (Some(path), Some("PropertiesChanged")) if path == session_path.as_str() => {
            publish_current_session_state(events, connection, session_path, lifecycle)?;
            None
        }
        _ => None,
    };
    if let Some(signal) = signal {
        publish(events, lifecycle.handle(signal));
    }
    Ok(())
}

fn publish_current_session_state(
    events: &PendingLifecycleEvents,
    connection: &Connection,
    session_path: &OwnedObjectPath,
    lifecycle: &mut SessionLifecycleModel,
) -> Result<()> {
    let session = session_proxy(connection, session_path)?;
    publish(
        events,
        lifecycle.handle(SessionLifecycleSignal::Active(
            session
                .get_property::<bool>("Active")
                .context("refresh login-session activity")?,
        )),
    );
    publish(
        events,
        lifecycle.handle(SessionLifecycleSignal::Locked(
            session
                .get_property::<bool>("LockedHint")
                .context("refresh login-session lock state")?,
        )),
    );
    Ok(())
}

fn session_proxy<'a>(
    connection: &'a Connection,
    session_path: &'a OwnedObjectPath,
) -> Result<Proxy<'a>> {
    Proxy::new(
        connection,
        LOGIN_DESTINATION,
        session_path.as_str(),
        SESSION_INTERFACE,
    )
    .context("create login-session proxy")
}

fn publish(events: &PendingLifecycleEvents, event: Option<ServiceEvent>) {
    if let Some(event) = event {
        events.push(event);
    }
}
