pragma Singleton
import Bootcraft
import QtQuick

// What the user picked, the plan for it and the state of a run. Pages read
// these properties and call these functions; only this file talks to Api.
QtObject {
    id: setup

    // ---- detected system and catalog -------------------------------------------
    property var system: null
    property var catalog: null
    readonly property bool ready: system !== null && catalog !== null
    property string loadError: ""

    // ---- choices ---------------------------------------------------------------
    /// "auto", "flatpak" or "native".
    property string preference: "auto"
    /// "" for the detected driver branch, else "modern", "legacy" or "none".
    property string nvidia: ""
    /// Workflow packs stacked on top of Essentials.
    property var packs: []
    /// "Custom": Essentials alone, apps picked by hand.
    property bool custom: false
    /// Apps added by hand, and pack apps removed by hand.
    property var appOn: ({})
    property var appOff: ({})
    /// Per-app source chosen with the badge: "flatpak" or "native".
    property var appSource: ({})
    /// Optional tasks ticked, and recommended (Essentials) tasks unticked.
    property var taskOn: ({})
    property var taskOff: ({})
    /// App id -> cached Flathub icon (file URL).
    property var appIcons: ({})

    // ---- derived ---------------------------------------------------------------
    readonly property var selectedApps: catalog ? catalog.apps.filter(a => appSelected(a.id)) : []
    readonly property var selectedTasks: catalog ? catalog.tasks.filter(t => taskSelected(t)) : []
    property var plan: null
    property string planError: ""
    /// Apps' download size: {bytes, partial}.
    property var downloadSize: null
    property int _sizeSerial: 0
    /// System tasks in the plan, including ones pulled in as requirements.
    readonly property var planSystemTasks: plan ? plan.tasks.filter(t => !t.isApp) : []
    readonly property int appCount: plan ? plan.appCount : selectedApps.length
    readonly property int systemCount: plan ? plan.systemCount : selectedTasks.length

    // ---- a run -----------------------------------------------------------------
    /// "idle", "authorizing" (password prompt), "running", "done" or "failed".
    property string runState: "idle"
    /// [{id, title, state, message}]; state is "pending", "running" or an outcome.
    property var runTasks: []
    property int finishedCount: 0
    property string runMessage: ""
    property bool rebootNeeded: false
    property string logPath: ""
    property bool cancelRequested: false
    property ListModel log: ListModel {}
    readonly property bool busy: runState === "authorizing" || runState === "running"

    /// Sidebar page: 0 Overview, 1 Applications, 2 System setup, 3 Activity, 4 Settings.
    property int page: 0

    property Timer _replanTimer: Timer {
        interval: 120
        onTriggered: setup._replan()
    }

    onSelectedAppsChanged: replan()
    onSelectedTasksChanged: replan()
    onAppSourceChanged: replan()
    onPreferenceChanged: reloadCatalog()
    onNvidiaChanged: load()

    Component.onCompleted: {
        Api.on("helper", e => setup._onHelper(e));
        Api.on("helper-stderr", e => setup.log.append({
                "kind": "stderr",
                "line": e.line
            }));
        Api.on("helper-exit", e => setup._onExit(e));
        Api.on("app-icon", e => {
            const icons = Object.assign({}, setup.appIcons);
            icons[e.id] = e.url;
            setup.appIcons = icons;
        });
    }

    // ---- loading ---------------------------------------------------------------
    function args() {
        const a = {
            "preference": preference
        };
        if (nvidia !== "")
            a.nvidia = nvidia;
        return a;
    }

    function load() {
        Api.call("system", args()).then(s => setup.system = s).catch(e => setup.loadError = String(e));
        reloadCatalog();
    }

    function reloadCatalog() {
        Api.call("catalog", args()).then(c => {
            const first = setup.catalog === null;
            setup.catalog = c;
            if (first)
                setup._fetchIcons();
            setup.replan();
        }).catch(e => setup.loadError = String(e));
    }

    function _fetchIcons() {
        const apps = catalog.apps.filter(a => a.flatpak).map(a => ({
                    "id": a.id,
                    "flatpak": a.flatpak.id
                }));
        Api.call("app_icons", {
            "apps": apps
        });
    }

    // ---- workflows and apps ----------------------------------------------------
    function pack(id) {
        return catalog ? catalog.packs.find(p => p.id === id) : null;
    }

    function packSelected(id) {
        if (id === "custom")
            return custom;
        if (id === "essentials")
            return true;
        return !custom && packs.indexOf(id) >= 0;
    }

    function togglePack(id) {
        if (id === "essentials")
            return;
        if (id === "custom") {
            custom = !custom;
            if (custom)
                appOff = {};
            return;
        }
        custom = false;
        const list = packs.slice();
        const i = list.indexOf(id);
        if (i >= 0)
            list.splice(i, 1);
        else
            list.push(id);
        packs = list;
    }

    function app(id) {
        return catalog ? catalog.apps.find(a => a.id === id) : null;
    }

    /// Ticked by default in one of the selected workflows.
    function inPacks(id) {
        if (!catalog || custom)
            return false;
        return catalog.packs.some(p => packs.indexOf(p.id) >= 0 && p.apps.some(a => a.id === id && a.ticked));
    }

    function appSelected(id) {
        const a = app(id);
        if (!a || !a.available)
            return false;
        return (inPacks(id) && !appOff[id]) || appOn[id] === true;
    }

    function toggleApp(id) {
        const on = Object.assign({}, appOn);
        const off = Object.assign({}, appOff);
        if (appSelected(id)) {
            if (inPacks(id))
                off[id] = true;
            delete on[id];
        } else {
            if (inPacks(id))
                delete off[id];
            else
                on[id] = true;
        }
        appOn = on;
        appOff = off;
    }

    function sourceOf(a) {
        return appSource[a.id] || (a.choice ? a.choice.source : "");
    }

    function sourceLabel(a) {
        const source = sourceOf(a);
        if (source === "flatpak")
            return "Flatpak";
        if (!a.native)
            return "";
        return ({
            "aur": "AUR",
            "signed-release": "Signed release",
            "rpm-fusion": "RPM Fusion"
        })[a.native.via] || "System";
    }

    /// `source` null goes back to the automatic choice.
    function setSource(id, source) {
        const sources = Object.assign({}, appSource);
        const a = app(id);
        if (!source || (a && a.choice && a.choice.source === source))
            delete sources[id];
        else
            sources[id] = source;
        appSource = sources;
    }

    // ---- system tasks ----------------------------------------------------------
    function task(id) {
        return catalog ? catalog.tasks.find(t => t.id === id) : null;
    }

    function taskSelected(t) {
        return t.recommended ? !taskOff[t.id] : taskOn[t.id] === true;
    }

    function toggleTask(id) {
        const t = task(id);
        if (!t)
            return;
        if (t.recommended) {
            const off = Object.assign({}, taskOff);
            if (off[id])
                delete off[id];
            else
                off[id] = true;
            taskOff = off;
        } else {
            const on = Object.assign({}, taskOn);
            if (on[id])
                delete on[id];
            else
                on[id] = true;
            taskOn = on;
        }
    }

    /// The name of whatever a plan entry's `requiredBy` points at.
    function titleOf(id) {
        if (id && id.startsWith("app.")) {
            const a = app(id.slice(4));
            return a ? a.name : id;
        }
        const t = task(id);
        return t ? t.title : id;
    }

    // ---- planning --------------------------------------------------------------
    function selection() {
        const s = args();
        s.tasks = selectedTasks.map(t => t.id);
        s.apps = selectedApps.map(a => appSource[a.id] ? {
                "id": a.id,
                "source": appSource[a.id]
            } : {
                "id": a.id
            });
        return s;
    }

    function replan() {
        _replanTimer.restart();
    }

    function _replan() {
        if (!catalog)
            return;
        const request = selection();
        Api.call("plan", request).then(p => {
            setup.plan = p;
            setup.planError = "";
        }).catch(e => {
            setup.plan = null;
            setup.planError = String(e);
        });
        // A newer request wins if the selection changes while this one runs.
        const serial = ++_sizeSerial;
        Api.call("download_size", request).then(size => {
            if (serial === setup._sizeSerial)
                setup.downloadSize = size;
        }).catch(() => {});
    }

    /// "about 3.2 GB", or "" before it's known or when there's nothing to download.
    function downloadLabel() {
        if (!downloadSize || downloadSize.bytes === 0)
            return "";
        const gb = downloadSize.bytes / 1e9;
        const amount = gb >= 1 ? gb.toFixed(1) + " GB" : Math.max(1, Math.round(downloadSize.bytes / 1e6)) + " MB";
        return (downloadSize.partial ? "about " : "") + amount;
    }

    function exportPlan(path) {
        const s = selection();
        s.path = path;
        return Api.call("export_plan", s);
    }

    // ---- running ---------------------------------------------------------------
    function start() {
        if (busy || !plan)
            return;
        runTasks = plan.tasks.map(t => ({
                    "id": t.id,
                    "title": t.title,
                    "state": "pending",
                    "message": ""
                }));
        log.clear();
        finishedCount = 0;
        runMessage = "";
        rebootNeeded = false;
        logPath = "";
        cancelRequested = false;
        runState = "authorizing";
        page = 3;
        Api.call("apply", selection()).catch(e => {
            setup.runState = "failed";
            setup.runMessage = String(e);
        });
    }

    function cancel() {
        cancelRequested = true;
        Api.call("cancel", {}).catch(() => {});
    }

    function reboot() {
        Api.call("reboot", {}).catch(e => setup.runMessage = String(e));
    }

    function _setTask(id, state, message) {
        runTasks = runTasks.map(t => t.id === id ? Object.assign({}, t, {
                "state": state,
                "message": message || ""
            }) : t);
    }

    function _onHelper(e) {
        switch (e.event) {
        case "plan":
            runState = "running";
            runTasks = e.tasks.map(t => ({
                        "id": t.id,
                        "title": t.title,
                        "state": "pending",
                        "message": ""
                    }));
            break;
        case "task-started":
            _setTask(e.id, "running");
            log.append({
                "kind": "task",
                "line": titleOf(e.id)
            });
            break;
        case "step":
            log.append({
                "kind": "step",
                "line": e.description
            });
            break;
        case "output":
            log.append({
                "kind": "output",
                "line": e.line
            });
            break;
        case "task-finished":
            _setTask(e.id, e.outcome, e.message);
            finishedCount += 1;
            if (e.outcome === "failed")
                log.append({
                    "kind": "error",
                    "line": titleOf(e.id) + ": " + e.message
                });
            break;
        case "finished":
            runState = e.ok ? "done" : "failed";
            rebootNeeded = e.reboot;
            logPath = e.log || "";
            if (!e.ok)
                runMessage = cancelRequested ? "Stopped before finishing." : "Some tasks did not complete.";
            break;
        case "error":
            runState = "failed";
            runMessage = e.message;
            break;
        }
    }

    function _onExit(e) {
        if (busy) {
            runState = "failed";
            runMessage = e.message || "The helper stopped before finishing.";
        }
        // Statuses (installed, done) changed.
        reloadCatalog();
    }
}
