//! Resolving AUR packages without an AUR helper.
//!
//! makepkg refuses to run as root, and `makepkg -si` or paru call sudo
//! themselves, which can't prompt under pkexec. So the helper resolves the
//! dependency tree here, installs repository dependencies with pacman, builds
//! each AUR package as the invoking user and installs it with `pacman -U`.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::probe::strip_version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AurInfo {
    pub name: String,
    /// The AUR git repository the package is built from.
    pub base: String,
    /// Runtime and build dependencies, without version constraints.
    pub depends: Vec<String>,
}

/// What resolution needs to know about the system and the AUR.
pub trait Resolver {
    /// Satisfied by an installed package (or something it provides).
    fn installed(&self, dep: &str) -> bool;
    /// Installable from the configured repositories (directly or via a provide).
    fn in_repos(&mut self, dep: &str) -> bool;
    /// AUR metadata for these names; names not in the AUR are left out.
    fn aur_info(&mut self, names: &[String]) -> Result<Vec<AurInfo>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AurPlan {
    /// Repository packages to install first, as dependencies.
    pub repo_deps: Vec<String>,
    /// AUR bases in build order, dependencies first.
    pub builds: Vec<Build>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    pub base: String,
    /// Packages from this base that were asked for.
    pub explicit: Vec<String>,
    /// Packages from this base needed only as dependencies.
    pub deps: Vec<String>,
}

pub fn resolve(resolver: &mut dyn Resolver, targets: &[String]) -> Result<AurPlan, String> {
    let mut known: HashMap<String, AurInfo> =
        resolver.aur_info(targets)?.into_iter().map(|info| (info.name.clone(), info)).collect();
    if let Some(missing) = targets.iter().find(|t| !known.contains_key(*t)) {
        return Err(format!("{missing} is not in the AUR"));
    }

    let mut walk = Walk::default();
    for target in targets {
        walk.visit(resolver, &mut known, target)?;
    }

    let mut builds: Vec<Build> = Vec::new();
    for name in walk.order {
        let base = &known[&name].base;
        let index = builds.iter().position(|b| &b.base == base).unwrap_or_else(|| {
            builds.push(Build { base: base.clone(), explicit: Vec::new(), deps: Vec::new() });
            builds.len() - 1
        });
        if targets.contains(&name) {
            builds[index].explicit.push(name);
        } else {
            builds[index].deps.push(name);
        }
    }
    Ok(AurPlan { repo_deps: walk.repo_deps, builds })
}

#[derive(Default)]
struct Walk {
    /// AUR packages, dependencies before the packages that need them.
    order: Vec<String>,
    visiting: HashSet<String>,
    repo_deps: Vec<String>,
}

impl Walk {
    fn visit(
        &mut self,
        resolver: &mut dyn Resolver,
        known: &mut HashMap<String, AurInfo>,
        name: &str,
    ) -> Result<(), String> {
        // `visiting` also stops dependency cycles.
        if !self.visiting.insert(name.to_owned()) {
            return Ok(());
        }
        for dep in known[name].depends.clone() {
            if self.order.contains(&dep) || self.repo_deps.contains(&dep) || resolver.installed(&dep) {
                continue;
            }
            if !known.contains_key(&dep) {
                if resolver.in_repos(&dep) {
                    self.repo_deps.push(dep);
                    continue;
                }
                let info = resolver.aur_info(std::slice::from_ref(&dep))?.pop().ok_or_else(|| {
                    format!("{name} needs {dep}, which is neither in the repositories nor in the AUR")
                })?;
                known.insert(dep.clone(), info);
            }
            self.visit(resolver, known, &dep)?;
        }
        self.order.push(name.to_owned());
        Ok(())
    }
}

/// AUR RPC v5 `info` URL for several packages.
pub fn rpc_url(names: &[String]) -> String {
    let query: Vec<String> = names.iter().map(|name| format!("arg[]={}", url_encode(name))).collect();
    format!("https://aur.archlinux.org/rpc/v5/info?{}", query.join("&"))
}

pub fn parse_rpc(body: &str) -> Result<Vec<AurInfo>, String> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        results: Vec<Package>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Package {
        name: String,
        package_base: String,
        #[serde(default)]
        depends: Vec<String>,
        #[serde(default)]
        make_depends: Vec<String>,
    }

    let response: Response =
        serde_json::from_str(body).map_err(|e| format!("unexpected AUR response: {e}"))?;
    if response.kind == "error" {
        return Err(format!("AUR error: {}", response.error.unwrap_or_default()));
    }
    Ok(response
        .results
        .into_iter()
        .map(|p| AurInfo {
            depends: p.depends.iter().chain(&p.make_depends).map(|d| strip_version(d).to_owned()).collect(),
            name: p.name,
            base: p.package_base,
        })
        .collect())
}

/// Package names are `[a-z0-9@._+-]`; `+` must not turn into a space.
fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'@' => char::from(b).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        installed: Vec<&'static str>,
        repos: Vec<&'static str>,
        aur: Vec<AurInfo>,
        queries: Vec<Vec<String>>,
    }

    impl Resolver for Fake {
        fn installed(&self, dep: &str) -> bool {
            self.installed.contains(&dep)
        }
        fn in_repos(&mut self, dep: &str) -> bool {
            self.repos.contains(&dep)
        }
        fn aur_info(&mut self, names: &[String]) -> Result<Vec<AurInfo>, String> {
            self.queries.push(names.to_vec());
            Ok(self.aur.iter().filter(|i| names.contains(&i.name)).cloned().collect())
        }
    }

    fn info(name: &str, base: &str, depends: &[&str]) -> AurInfo {
        AurInfo {
            name: name.into(),
            base: base.into(),
            depends: depends.iter().map(|d| (*d).to_owned()).collect(),
        }
    }

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn builds_aur_dependencies_first_and_collects_repo_deps() {
        let mut fake = Fake {
            installed: vec!["hicolor-icon-theme"],
            repos: vec!["lightdm-slick-greeter", "python"],
            aur: vec![
                info(
                    "mint-artwork",
                    "mint-artwork",
                    &["hicolor-icon-theme", "lightdm-slick-greeter", "mint-themes", "mint-y-icons"],
                ),
                info("mint-themes", "mint-themes", &["python"]),
                info("mint-y-icons", "mint-y-icons", &["mint-themes"]),
            ],
            queries: Vec::new(),
        };
        let plan = resolve(&mut fake, &names(&["mint-artwork"])).unwrap();
        assert_eq!(plan.repo_deps, ["lightdm-slick-greeter", "python"]);
        let order: Vec<_> = plan.builds.iter().map(|b| b.base.as_str()).collect();
        assert_eq!(order, ["mint-themes", "mint-y-icons", "mint-artwork"]);
        assert_eq!(plan.builds[2].explicit, ["mint-artwork"]);
        assert_eq!(plan.builds[0].deps, ["mint-themes"]);
    }

    #[test]
    fn targets_that_depend_on_each_other_are_ordered_without_extra_queries() {
        let mut fake = Fake {
            installed: vec![],
            repos: vec!["dkms", "libglvnd"],
            aur: vec![
                info("nvidia-580xx-utils", "nvidia-580xx-utils", &["libglvnd"]),
                info("lib32-nvidia-580xx-utils", "lib32-nvidia-580xx-utils", &[]),
                info("nvidia-580xx-dkms", "nvidia-580xx-utils", &["dkms", "nvidia-580xx-utils", "libglvnd"]),
            ],
            queries: Vec::new(),
        };
        let targets = names(&["nvidia-580xx-dkms", "lib32-nvidia-580xx-utils", "nvidia-580xx-utils"]);
        let plan = resolve(&mut fake, &targets).unwrap();
        assert_eq!(fake.queries.len(), 1);
        assert_eq!(plan.repo_deps, ["dkms", "libglvnd"]);
        // Split packages from one base are built once.
        assert_eq!(plan.builds.len(), 2);
        assert_eq!(plan.builds[0].base, "nvidia-580xx-utils");
        assert_eq!(plan.builds[0].explicit, ["nvidia-580xx-utils", "nvidia-580xx-dkms"]);
    }

    #[test]
    fn unresolvable_dependency_is_an_error() {
        let mut fake = Fake {
            installed: vec![],
            repos: vec![],
            aur: vec![info("a", "a", &["ghost"])],
            queries: Vec::new(),
        };
        let err = resolve(&mut fake, &names(&["a"])).unwrap_err();
        assert!(err.contains("ghost"), "{err}");
        assert_eq!(resolve(&mut fake, &names(&["nope"])).unwrap_err(), "nope is not in the AUR");
    }

    #[test]
    fn rpc_url_and_parsing() {
        assert_eq!(
            rpc_url(&names(&["paru-bin", "libsigc++"])),
            "https://aur.archlinux.org/rpc/v5/info?arg[]=paru-bin&arg[]=libsigc%2B%2B"
        );
        let body = r#"{"resultcount":1,"results":[{"Name":"nvidia-580xx-dkms","PackageBase":"nvidia-580xx-utils","Depends":["dkms","nvidia-580xx-utils=580.196.07"],"MakeDepends":["git"]}],"type":"multiinfo","version":5}"#;
        assert_eq!(
            parse_rpc(body).unwrap(),
            [info("nvidia-580xx-dkms", "nvidia-580xx-utils", &["dkms", "nvidia-580xx-utils", "git"])]
        );
        assert!(parse_rpc(r#"{"type":"error","error":"Too many package results.","results":[]}"#).is_err());
    }
}
