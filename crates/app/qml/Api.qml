pragma Singleton
import Bootcraft
import QtQuick

// Promise-based calls into the Rust backend, and its events.
//   Api.call("plan", selection).then(plan => ...).catch(message => ...)
//   Api.on("helper", event => ...)   // returns an unsubscribe function
QtObject {
    id: api

    property var _pending: ({})
    property var _listeners: ({})
    property Connections _connections: Connections {
        target: Backend

        function onReplied(requestId, ok, json) {
            const pending = api._pending[requestId];
            if (!pending)
                return;
            delete api._pending[requestId];
            const value = api._parse(json);
            if (ok)
                pending.resolve(value);
            else
                pending.reject(value);
        }

        function onBackendEvent(name, json) {
            const payload = api._parse(json);
            (api._listeners[name] || []).forEach(handler => handler(payload));
        }
    }

    function call(command, args) {
        return new Promise(function (resolve, reject) {
            const id = Backend.invoke(command, JSON.stringify(args || {}));
            api._pending[id] = {
                "resolve": resolve,
                "reject": reject
            };
        });
    }

    function on(name, handler) {
        const list = api._listeners[name] || [];
        list.push(handler);
        api._listeners[name] = list;
        return function () {
            api._listeners[name] = (api._listeners[name] || []).filter(h => h !== handler);
        };
    }

    function _parse(json) {
        try {
            return JSON.parse(json);
        } catch (_) {
            return json;
        }
    }
}
