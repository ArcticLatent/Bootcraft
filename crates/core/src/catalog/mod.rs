//! The tasks Bootcraft offers on each distribution, in execution order.

mod arch;

use crate::system::{Distro, System};
use crate::task::Task;

pub const FLATHUB_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// Tasks for this system. Distributions that have not been ported yet get an
/// empty list.
pub fn catalog(sys: &System) -> Vec<Task> {
    match sys.distro() {
        Some(Distro::Arch) => arch::tasks(sys),
        Some(Distro::Fedora | Distro::Ubuntu | Distro::Mint | Distro::Debian) | None => Vec::new(),
    }
}
