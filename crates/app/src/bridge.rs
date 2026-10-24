//! The QML side's door into Rust: the `Backend` singleton.
//!
//! `Backend.invoke(command, argsJson)` runs a command off the UI thread and
//! returns a request id; the result arrives later as `replied(id, ok, json)`.
//! Events (the helper's progress, app icons) arrive as
//! `backendEvent(name, json)`. `qml/Api.qml` wraps both into promises and
//! listeners. The same shape as ComfyPilot's bridge.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Backend = super::BackendRust;

        /// Starts `command` with a JSON object of arguments; returns the request id.
        #[qinvokable]
        fn invoke(self: Pin<&mut Backend>, command: &QString, args_json: &QString) -> i32;

        /// The result of request `request_id`: a JSON value, or a JSON string error.
        #[qsignal]
        fn replied(self: Pin<&mut Backend>, request_id: i32, ok: bool, json: QString);

        /// An event from the backend, with a JSON payload.
        #[qsignal]
        #[cxx_name = "backendEvent"]
        fn backend_event(self: Pin<&mut Backend>, name: QString, json: QString);
    }

    impl cxx_qt::Threading for Backend {}
    impl cxx_qt::Initialize for Backend {}
}

use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::Value;

#[derive(Default)]
pub struct BackendRust {
    next_id: AtomicI32,
}

/// Where events go: the `Backend` object once QML has created it; until
/// then they wait and are delivered when it appears.
enum EventTarget {
    Waiting(Vec<(String, Value)>),
    Ready(CxxQtThread<qobject::Backend>),
}

static EVENT_TARGET: Mutex<EventTarget> = Mutex::new(EventTarget::Waiting(Vec::new()));

fn deliver(thread: &CxxQtThread<qobject::Backend>, name: String, payload: &Value) {
    let json = payload.to_string();
    let _ = thread.queue(move |backend| backend.backend_event(QString::from(&name), QString::from(&json)));
}

/// Send an event to QML from any thread.
pub fn push_event(name: &str, payload: Value) {
    let Ok(mut target) = EVENT_TARGET.lock() else { return };
    match &mut *target {
        EventTarget::Ready(thread) => deliver(thread, name.to_owned(), &payload),
        EventTarget::Waiting(queue) => queue.push((name.to_owned(), payload)),
    }
}

impl cxx_qt::Initialize for qobject::Backend {
    fn initialize(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        if let Ok(mut target) = EVENT_TARGET.lock() {
            if let EventTarget::Waiting(queue) = &mut *target {
                for (name, payload) in queue.drain(..) {
                    deliver(&thread, name, &payload);
                }
            }
            *target = EventTarget::Ready(thread);
        }
    }
}

impl qobject::Backend {
    pub fn invoke(self: Pin<&mut Self>, command: &QString, args_json: &QString) -> i32 {
        let id = self.as_ref().rust().next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let thread = self.qt_thread();
        let command = command.to_string();
        let args: Value = serde_json::from_str(&args_json.to_string()).unwrap_or(Value::Null);
        std::thread::spawn(move || {
            let (ok, json) = match crate::commands::dispatch(&command, &args) {
                Ok(value) => (true, value.to_string()),
                Err(message) => (false, Value::String(message).to_string()),
            };
            let _ = thread.queue(move |backend| backend.replied(id, ok, QString::from(&json)));
        });
        id
    }
}
