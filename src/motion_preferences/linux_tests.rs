//! Owned peer-to-peer D-Bus fixtures; run the exact production consumer.
use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::Notify;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.portal.Error")]
enum PortalError {
    NotFound(String),
}
#[derive(Default)]
struct Values {
    standard: Option<u32>,
    gnome: Option<bool>,
    invalid_standard: bool,
}
#[derive(Default)]
struct Fixture {
    values: Mutex<Values>,
    calls: AtomicUsize,
    stall: AtomicBool,
    entered: Notify,
    release: Notify,
    completed: Notify,
}
impl Fixture {
    async fn read(&self, namespace: &str, key: &str) -> Result<OwnedValue, PortalError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.stall.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
            self.completed.notify_one();
        }
        let values = self.values.lock().unwrap();
        if (namespace, key) == Key::Standard.names() {
            if values.invalid_standard {
                return Ok(OwnedValue::from(false));
            }
            if let Some(value) = values.standard {
                return Ok(OwnedValue::from(value));
            }
        }
        if (namespace, key) == Key::Gnome.names()
            && let Some(value) = values.gnome
        {
            return Ok(OwnedValue::from(value));
        }
        Err(PortalError::NotFound(
            "self-authored missing setting".into(),
        ))
    }
}
struct Modern(Arc<Fixture>);
#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl Modern {
    async fn read_one(&self, namespace: &str, key: &str) -> Result<OwnedValue, PortalError> {
        self.0.read(namespace, key).await
    }
}
struct Legacy(Arc<Fixture>);
#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl Legacy {
    async fn read(&self, namespace: &str, key: &str) -> Result<OwnedValue, PortalError> {
        let value = self.0.read(namespace, key).await?;
        Ok(OwnedValue::try_from(Value::Value(Box::new(value.into()))).unwrap())
    }
}
async fn pair(service: impl zbus::object_server::Interface) -> (Connection, Connection) {
    let (client, server) = tokio::net::UnixStream::pair().unwrap();
    let server = zbus::connection::Builder::unix_stream(server)
        .p2p()
        .server(zbus::Guid::generate())
        .unwrap()
        .serve_at(PATH, service)
        .unwrap()
        .build();
    let client = zbus::connection::Builder::unix_stream(client)
        .p2p()
        .max_queued(8)
        .build();
    let (client, server) = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(client, server)
    })
    .await
    .unwrap();
    (client.unwrap(), server.unwrap())
}
fn launch(connection: Connection) -> (Provider, watch::Receiver<Preference>) {
    let (sender, values) = watch::channel(Preference::Pending);
    let (stop, mut cancel) = oneshot::channel();
    let task = tokio::spawn(async move { session(connection, sender, &mut cancel).await });
    (
        Provider {
            stop: Some(stop),
            task: Some(task),
        },
        values,
    )
}
async fn next(
    values: &mut watch::Receiver<Preference>,
    condition: impl Fn(&Preference) -> bool,
) -> Preference {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let value = values.borrow_and_update().clone();
            if condition(&value) {
                return value;
            }
            values.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}
async fn signal(connection: &Connection, key: Key, value: OwnedValue) {
    let (namespace, key) = key.names();
    connection
        .emit_signal(
            None::<&str>,
            PATH,
            INTERFACE,
            "SettingChanged",
            &(namespace, key, value),
        )
        .await
        .unwrap();
}
async fn close(
    mut provider: Provider,
    mut values: watch::Receiver<Preference>,
    server: Connection,
) {
    let audit = tokio::time::timeout(Duration::from_secs(2), provider.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(audit.listener_removed);
    assert_eq!(audit.worker_joined, Some(true));
    assert!(
        tokio::time::timeout(Duration::from_secs(1), values.changed())
            .await
            .unwrap()
            .is_err()
    );
    tokio::time::timeout(Duration::from_secs(1), server.closed())
        .await
        .unwrap();
}

#[tokio::test]
async fn standard_precedence_and_an_old_signal_cannot_revert_the_current_value() {
    let state = Arc::new(Fixture::default());
    *state.values.lock().unwrap() = Values {
        standard: Some(0),
        gnome: Some(false),
        invalid_standard: false,
    };
    let (client, server) = pair(Modern(state.clone())).await;
    let (provider, mut values) = launch(client);
    assert_eq!(
        next(&mut values, |v| !matches!(v, Preference::Pending)).await,
        Preference::Known {
            reduced: false,
            source: "linux_portal_reduced_motion",
            raw_value: Some(0),
        }
    );
    state.values.lock().unwrap().standard = Some(1);
    // Deliberately stale payload: the source must reread, not paint this bool.
    signal(&server, Key::Standard, OwnedValue::from(0_u32)).await;
    assert_eq!(
        next(&mut values, |v| matches!(
            v,
            Preference::Known { reduced: true, .. }
        ))
        .await,
        Preference::Known {
            reduced: true,
            source: "linux_portal_reduced_motion",
            raw_value: Some(1),
        }
    );
    let before = state.calls.load(Ordering::SeqCst);
    server
        .emit_signal(
            None::<&str>,
            PATH,
            INTERFACE,
            "SettingChanged",
            &(
                "unrelated.namespace",
                "other",
                OwnedValue::try_from(Value::from("x".repeat(8192))).unwrap(),
            ),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!values.has_changed().unwrap());
    assert_eq!(state.calls.load(Ordering::SeqCst), before);
    close(provider, values, server).await;
}
#[tokio::test]
async fn an_old_portal_uses_the_actual_legacy_double_variant_and_gnome_key() {
    let state = Arc::new(Fixture::default());
    state.values.lock().unwrap().gnome = Some(false);
    let (client, server) = pair(Legacy(state.clone())).await;
    let (provider, mut values) = launch(client);
    assert_eq!(
        next(&mut values, |v| !matches!(v, Preference::Pending)).await,
        Preference::Known {
            reduced: true,
            source: "linux_portal_gnome_animations",
            raw_value: None,
        }
    );
    state.values.lock().unwrap().gnome = Some(true);
    signal(&server, Key::Gnome, OwnedValue::from(false)).await;
    assert_eq!(
        next(&mut values, |v| matches!(
            v,
            Preference::Known { reduced: false, .. }
        ))
        .await,
        Preference::Known {
            reduced: false,
            source: "linux_portal_gnome_animations",
            raw_value: None,
        }
    );
    close(provider, values, server).await;
}
#[tokio::test]
async fn malformed_standard_value_is_unknown_and_recovers_on_a_real_signal() {
    let state = Arc::new(Fixture::default());
    *state.values.lock().unwrap() = Values {
        standard: None,
        gnome: Some(true),
        invalid_standard: true,
    };
    let (client, server) = pair(Modern(state.clone())).await;
    let (provider, mut values) = launch(client);
    let reading = next(&mut values, |v| !matches!(v, Preference::Pending)).await;
    assert!(matches!(
        reading,
        Preference::Unavailable {
            stage: "read_standard",
            ..
        }
    ));
    assert!(reading.reduced());
    {
        let mut v = state.values.lock().unwrap();
        v.invalid_standard = false;
        v.standard = Some(0);
    }
    signal(&server, Key::Standard, OwnedValue::from(0_u32)).await;
    next(&mut values, |v| {
        matches!(v, Preference::Known { reduced: false, .. })
    })
    .await;
    close(provider, values, server).await;
}
#[tokio::test]
async fn closing_cancels_a_read_without_waiting_for_the_portal_response_and_joins() {
    let state = Arc::new(Fixture::default());
    state.stall.store(true, Ordering::SeqCst);
    let (client, server) = pair(Modern(state.clone())).await;
    let (mut provider, mut values) = launch(client);
    tokio::time::timeout(Duration::from_secs(2), state.entered.notified())
        .await
        .unwrap();
    let audit = tokio::time::timeout(Duration::from_secs(1), provider.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(audit.worker_joined, Some(true));
    assert!(audit.listener_removed);
    assert!(values.changed().await.is_err());
    state.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), state.completed.notified())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), server.closed())
        .await
        .unwrap();
    drop(server);
}
