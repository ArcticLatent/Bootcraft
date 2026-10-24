//! Messages between the GUI and `bootcraft-helper serve`: one JSON object per
//! line. The GUI writes a [`Request`] (and later, optionally, a [`Control`]) to
//! the helper's stdin; the helper answers with [`Event`]s on stdout.

use serde::{Deserialize, Serialize};

use crate::apps::Preference;
use crate::plan::AppPick;
use crate::system::{NvidiaGen, System};

/// What to run. The helper re-detects the system itself and only accepts task
/// IDs from its own catalog; the desktop and session come from the GUI because
/// the helper runs outside the user's session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// System task IDs.
    pub select: Vec<String>,
    #[serde(default)]
    pub apps: Vec<AppPick>,
    #[serde(default)]
    pub preference: Preference,
    #[serde(default)]
    pub desktop: Option<String>,
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub nvidia: Option<NvidiaChoice>,
}

/// The user's override of the detected NVIDIA driver branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NvidiaChoice {
    Modern,
    Legacy,
    None,
}

impl NvidiaChoice {
    pub fn apply(self, sys: &mut System) {
        sys.nvidia = match self {
            Self::Modern => Some(NvidiaGen::Modern),
            Self::Legacy => Some(NvidiaGen::Legacy580),
            Self::None => None,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "control")]
pub enum Control {
    /// Stop after the step that is running now.
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "event")]
pub enum Event {
    /// The resolved plan, sent once before anything runs.
    Plan {
        tasks: Vec<PlanEntry>,
    },
    TaskStarted {
        id: String,
    },
    /// A step of the running task begins.
    Step {
        id: String,
        description: String,
    },
    /// A line of command output, or a note from the executor.
    Output {
        line: String,
    },
    TaskFinished {
        id: String,
        outcome: Outcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Everything has run (or stopped).
    Finished {
        ok: bool,
        reboot: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        log: Option<String>,
    },
    /// The request could not be run at all.
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Done,
    /// Nothing to do; the task was already in effect.
    AlreadyDone,
    Failed,
    /// Not run because a task it requires failed.
    Skipped,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_defaults_and_round_trip() {
        let request: Request = serde_json::from_str(r#"{"select":["system.update"]}"#).unwrap();
        assert_eq!(request.select, ["system.update"]);
        assert_eq!(request.nvidia, None);

        let event = Event::TaskFinished { id: "x".into(), outcome: Outcome::AlreadyDone, message: None };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"event":"task-finished","id":"x","outcome":"already-done"}"#);
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);

        let cancel: Control = serde_json::from_str(r#"{"control":"cancel"}"#).unwrap();
        assert_eq!(cancel, Control::Cancel);
    }
}
