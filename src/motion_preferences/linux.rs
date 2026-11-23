use super::{Audit, Preference};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::{
    sync::{oneshot, watch},
    task::JoinHandle,
};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedValue, Value},
};

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BODY: usize = 4096;
const INTERFACE: &str = "org.freedesktop.portal.Settings";
const PATH: &str = "/org/freedesktop/portal/desktop";
const DESTINATION: &str = "org.freedesktop.portal.Desktop";

#[derive(Clone, Copy)]
enum Key {
    Standard,
    Gnome,
}
struct Parsed {
    reduced: bool,
    raw_value: Option<u32>,
}
impl Key {
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Standard => ("org.freedesktop.appearance", "reduced-motion"),
            Self::Gnome => ("org.gnome.desktop.interface", "enable-animations"),
        }
    }
    fn parse(self, value: &Value<'_>) -> Result<Parsed, String> {
        match self {
            // The portal specification maps unknown u32 values to no preference.
            Self::Standard => u32::try_from(value)
                .map(|v| Parsed {
                    reduced: v == 1,
                    raw_value: Some(v),
                })
                .map_err(|e| e.to_string()),
            Self::Gnome => bool::try_from(value)
                .map(|enabled| Parsed {
                    reduced: !enabled,
                    raw_value: None,
                })
                .map_err(|e| e.to_string()),
        }
    }
}
fn method_error(error: &zbus::Error, name: &str) -> bool {
    matches!(error,zbus::Error::MethodError(actual,_,_) if actual.as_str()==name)
}
async fn read(proxy: &Proxy<'_>, key: Key) -> Result<Option<Parsed>, String> {
    let call = |method| async move {
        tokio::time::timeout(TIMEOUT, proxy.call_method(method, &key.names()))
            .await
            .map_err(|_| "motion setting read timed out".to_owned())
    };
    let (result, legacy) = match call("ReadOne").await? {
        Err(error) if method_error(&error, "org.freedesktop.DBus.Error.UnknownMethod") => {
            (call("Read").await?, true)
        }
        other => (other, false),
    };
    let reply = match result {
        Ok(reply) => reply,
        Err(error)
            if method_error(&error, "org.freedesktop.portal.Error.NotFound")
                || method_error(&error, "org.freedesktop.DBus.Error.UnknownMethod") =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error.to_string()),
    };
    if reply.body().len() > MAX_BODY {
        return Err("motion setting body exceeds limit".into());
    }
    let value: OwnedValue = reply.body().deserialize().map_err(|e| e.to_string())?;
    let value = if legacy {
        let Value::Value(inner) = &*value else {
            return Err("legacy motion setting must contain two variant layers".into());
        };
        inner.as_ref()
    } else {
        &*value
    };
    key.parse(value).map(Some)
}
async fn refresh(proxy: &Proxy<'_>) -> Preference {
    match read(proxy, Key::Standard).await {
        Ok(Some(value)) => Preference::Known {
            reduced: value.reduced,
            source: "linux_portal_reduced_motion",
            raw_value: value.raw_value,
        },
        Err(error) => Preference::failed("read_standard", error),
        Ok(None) => match read(proxy, Key::Gnome).await {
            Ok(Some(value)) => Preference::Known {
                reduced: value.reduced,
                source: "linux_portal_gnome_animations",
                raw_value: value.raw_value,
            },
            Ok(None) => Preference::failed("unsupported_key", "portal has no supported motion key"),
            Err(error) => Preference::failed("read_compatibility", error),
        },
    }
}
fn relevant(message: &zbus::Message) -> Result<bool, String> {
    if message.body().len() > MAX_BODY {
        return Err("motion signal body exceeds limit".into());
    }
    let body = message.body();
    let (namespace, key, _): (&str, &str, Value<'_>) =
        body.deserialize().map_err(|e| e.to_string())?;
    Ok([Key::Standard.names(), Key::Gnome.names()].contains(&(namespace, key)))
}
async fn watch_connection(
    connection: &Connection,
    sender: &watch::Sender<Preference>,
) -> Result<(), String> {
    let proxy = Proxy::new(connection, DESTINATION, PATH, INTERFACE)
        .await
        .map_err(|e| e.to_string())?;
    let standard = Key::Standard.names();
    let gnome = Key::Gnome.names();
    let standard = tokio::time::timeout(
        TIMEOUT,
        proxy.receive_signal_with_args("SettingChanged", &[(0, standard.0), (1, standard.1)]),
    )
    .await
    .map_err(|_| "motion subscription timed out")?
    .map_err(|e| e.to_string())?;
    let gnome = tokio::time::timeout(
        TIMEOUT,
        proxy.receive_signal_with_args("SettingChanged", &[(0, gnome.0), (1, gnome.1)]),
    )
    .await
    .map_err(|_| "motion compatibility subscription timed out")?
    .map_err(|e| e.to_string())?;
    let mut signals = futures_util::stream::select(standard, gnome);
    // Subscribe before the initial read. Signals are invalidations, not cached
    // values: rereading also prevents an older queued signal from overwriting
    // a newer initial/method reply. There is no timer between real OS events.
    sender.send_replace(refresh(&proxy).await);
    while let Some(message) = signals.next().await {
        match relevant(&message) {
            Ok(true) => {
                sender.send_replace(refresh(&proxy).await);
            }
            Ok(false) => {}
            Err(error) => {
                sender.send_replace(Preference::failed("signal", error));
            }
        }
    }
    Err("motion setting signal stream closed".into())
}
async fn worker(
    sender: watch::Sender<Preference>,
    mut stop: oneshot::Receiver<()>,
) -> Result<Audit, String> {
    let connect = async {
        zbus::connection::Builder::session()?
            .max_queued(8)
            .build()
            .await
    };
    let connection = tokio::select! {biased;
        _=&mut stop=>return Ok(Audit{listener_removed:true,worker_joined:Some(true)}),
        result=tokio::time::timeout(TIMEOUT,connect)=>match result {
            Ok(Ok(connection))=>connection,
            Ok(Err(error))=>{sender.send_replace(Preference::failed("session_bus",error));return Ok(Audit{listener_removed:true,worker_joined:Some(true)});},
            Err(_)=>{sender.send_replace(Preference::failed("session_bus","motion bus connection timed out"));return Ok(Audit{listener_removed:true,worker_joined:Some(true)});},
        }
    };
    session(connection, sender, &mut stop).await
}
async fn session(
    connection: Connection,
    sender: watch::Sender<Preference>,
    stop: &mut oneshot::Receiver<()>,
) -> Result<Audit, String> {
    let result =
        tokio::select! {biased;_=stop=>Ok(()),result=watch_connection(&connection,&sender)=>result};
    if let Err(error) = result {
        sender.send_replace(Preference::failed("watch", error));
    }
    tokio::time::timeout(TIMEOUT, connection.close())
        .await
        .map_err(|_| "motion bus close timed out")?
        .map_err(|e| e.to_string())?;
    Ok(Audit {
        listener_removed: true,
        worker_joined: Some(true),
    })
}
pub struct Provider {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<Audit, String>>>,
}
impl Provider {
    pub fn start(sender: watch::Sender<Preference>) -> Result<Self, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let (stop, receive) = oneshot::channel();
        let task = runtime.spawn(worker(sender, receive));
        Ok(Self {
            stop: Some(stop),
            task: Some(task),
        })
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        Ok(())
    }
    pub async fn shutdown(&mut self) -> Result<Audit, String> {
        self.cancel()?;
        let Some(task) = self.task.as_mut() else {
            return Err("motion worker was already consumed".into());
        };
        let result = task.await.map_err(|e| e.to_string())?;
        self.task.take();
        result
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        let _ = self.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_values_and_the_standard_unknown_value_rule_are_independent() {
        assert!(Key::Standard.parse(&Value::U32(1)).unwrap().reduced);
        for n in [0, 2, u32::MAX] {
            let value = Key::Standard.parse(&Value::U32(n)).unwrap();
            assert!(!value.reduced);
            assert_eq!(value.raw_value, Some(n));
        }
        assert!(Key::Standard.parse(&Value::Bool(true)).is_err());
        assert!(Key::Gnome.parse(&Value::Bool(false)).unwrap().reduced);
        assert!(!Key::Gnome.parse(&Value::Bool(true)).unwrap().reduced);
        assert!(Key::Gnome.parse(&Value::U32(1)).is_err());
    }
}

#[cfg(test)]
#[path = "linux_tests.rs"]
mod protocol_tests;
