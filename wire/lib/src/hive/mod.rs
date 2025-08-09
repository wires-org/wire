use node::{Name, Node};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::OccupiedEntry;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, trace};

use crate::errors::{HiveInitializationError, NixChildError};
use crate::nix::{EvalGoal, get_eval_command};
use crate::{HiveLibError, SubCommandModifiers};
pub mod node;
pub mod steps;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Hive {
    pub nodes: HashMap<Name, Node>,

    #[serde(deserialize_with = "check_schema_version", rename = "_schema")]
    pub schema: u32,
}

pub enum Action<'a> {
    Inspect,
    EvaluateNode(OccupiedEntry<'a, String, Node>),
}

fn check_schema_version<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(d)?;
    if version != Hive::SCHEMA_VERSION {
        return Err(D::Error::custom(
            "Version mismatch for Hive. Please ensure the binary and your wire input match!",
        ));
    }
    Ok(version)
}

impl Hive {
    pub const SCHEMA_VERSION: u32 = 0;

    #[instrument]
    pub async fn new_from_path(
        path: &Path,
        modifiers: SubCommandModifiers,
    ) -> Result<Hive, HiveLibError> {
        info!("Searching upwards for hive in {}", path.display());

        let command = get_eval_command(path, &EvalGoal::Inspect, modifiers)?
            .output()
            .await
            .map_err(|err| HiveLibError::NixChildError(NixChildError::ResolveError(err)))?;

        let stdout = String::from_utf8_lossy(&command.stdout);
        let stderr = String::from_utf8_lossy(&command.stderr);

        debug!("Output of nix eval: {stdout}");

        if command.status.success() {
            let hive: Hive = serde_json::from_str(&stdout).map_err(|err| {
                HiveLibError::HiveInitializationError(HiveInitializationError::ParseEvaluateError(
                    err,
                ))
            })?;

            return Ok(hive);
        }

        Err(HiveLibError::HiveInitializationError(
            HiveInitializationError::NixEvalError(
                stderr
                    .split('\n')
                    .map(std::string::ToString::to_string)
                    .collect(),
            ),
        ))
    }

    /// # Errors
    ///
    /// Returns an error if a node in nodes does not exist in the hive.
    pub fn force_always_local(&mut self, nodes: Vec<String>) -> Result<(), HiveLibError> {
        for node in nodes {
            info!("Forcing a local build for {node}");

            self.nodes
                .get_mut(&Name(Arc::from(node.clone())))
                .ok_or(HiveLibError::HiveInitializationError(
                    HiveInitializationError::NodeDoesNotExist(node.to_string()),
                ))?
                .build_remotely = false;
        }

        Ok(())
    }
}

pub fn find_hive(path: &Path) -> Option<PathBuf> {
    trace!("Searching for hive in {}", path.display());
    let filepath_flake = path.join("flake.nix");

    if filepath_flake.is_file() {
        return Some(filepath_flake);
    }
    let filepath_hive = path.join("hive.nix");

    if filepath_hive.is_file() {
        return Some(filepath_hive);
    }

    if let Some(parent) = path.parent() {
        return find_hive(parent);
    }

    error!("No hive found");
    None
}

#[cfg(test)]
mod tests {
    use im::vector;

    use crate::{
        get_test_path,
        hive::steps::keys::{Key, Source, UploadKeyAt},
        test_support::make_flake_sandbox,
    };

    use super::*;
    use std::env;

    #[test]
    fn test_hive_dot_nix_priority() {
        let path = get_test_path!();

        let hive = find_hive(&path).unwrap();

        assert!(hive.ends_with("flake.nix"));
    }

    #[tokio::test]
    #[cfg_attr(feature = "no_web_tests", ignore)]
    async fn test_hive_file() {
        let mut path = get_test_path!();

        let hive = Hive::new_from_path(&path, SubCommandModifiers::default())
            .await
            .unwrap();

        let node = Node {
            target: node::Target::from_host("192.168.122.96"),
            ..Default::default()
        };

        let mut nodes = HashMap::new();
        nodes.insert(Name("node-a".into()), node);

        path.push("hive.nix");

        assert_eq!(
            hive,
            Hive {
                nodes,
                schema: Hive::SCHEMA_VERSION
            }
        );
    }

    #[tokio::test]
    #[cfg_attr(feature = "no_web_tests", ignore)]
    async fn non_trivial_hive() {
        let mut path = get_test_path!();

        let hive = Hive::new_from_path(&path, SubCommandModifiers::default())
            .await
            .unwrap();

        let node = Node {
            target: node::Target::from_host("name"),
            keys: vector![Key {
                name: "different-than-a".into(),
                dest_dir: "/run/keys/".into(),
                path: "/run/keys/different-than-a".into(),
                group: "root".into(),
                user: "root".into(),
                permissions: "0600".into(),
                source: Source::String("hi".into()),
                upload_at: UploadKeyAt::PreActivation,
            }],
            ..Default::default()
        };

        let mut nodes = HashMap::new();
        nodes.insert(Name("node-a".into()), node);

        path.push("hive.nix");

        assert_eq!(
            hive,
            Hive {
                nodes,
                schema: Hive::SCHEMA_VERSION
            }
        );
    }

    #[tokio::test]
    #[cfg_attr(feature = "no_web_tests", ignore)]
    async fn flake_hive() {
        let tmp_dir = make_flake_sandbox(&get_test_path!()).unwrap();

        let hive = Hive::new_from_path(tmp_dir.path(), SubCommandModifiers::default())
            .await
            .unwrap();

        let mut nodes = HashMap::new();

        // a merged node
        nodes.insert(Name("node-a".into()), Node::from_host("node-a"));
        // a non-merged node
        nodes.insert(Name("node-b".into()), Node::from_host("node-b"));
        // omit a node called system-c

        let mut path = tmp_dir.path().to_path_buf();
        path.push("flake.nix");

        assert_eq!(
            hive,
            Hive {
                nodes,
                schema: Hive::SCHEMA_VERSION
            }
        );

        tmp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn no_nixpkgs() {
        let path = get_test_path!();

        assert!(matches!(
            Hive::new_from_path(&path, SubCommandModifiers::default()).await,
            Err(HiveLibError::NixEvalError(..))
        ));
    }

    #[tokio::test]
    async fn _keys_should_fail() {
        let path = get_test_path!();

        assert!(matches!(
            Hive::new_from_path(&path, SubCommandModifiers::default()).await,
            Err(HiveLibError::NixEvalError(..))
        ));
    }
}
