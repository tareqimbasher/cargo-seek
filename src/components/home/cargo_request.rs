//! The add/install request lifecycle: deciding whether a crate's features must be chosen before
//! running cargo, and deferring that decision until feature metadata has loaded.

use crate::action::Action;
use crate::cargo::CargoCommand;
use crate::components::home::feature_selector::FeatureSelector;
use crate::config::Config;
use crate::search::Crate;

/// Which cargo action the user initiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoIntent {
    Add,
    Install,
}

impl CargoIntent {
    /// The verb shown in the picker title.
    pub fn verb(self) -> &'static str {
        match self {
            CargoIntent::Add => "Add",
            CargoIntent::Install => "Install",
        }
    }

    /// Builds the cargo command for this intent.
    pub fn into_command(
        self,
        name: String,
        version: String,
        features: Vec<String>,
        no_default_features: bool,
    ) -> Action {
        let command = match self {
            CargoIntent::Add => CargoCommand::Add {
                name,
                version,
                features,
                no_default_features,
            },
            CargoIntent::Install => CargoCommand::Install {
                name,
                version,
                features,
                no_default_features,
            },
        };
        Action::Cargo(command)
    }
}

/// An add/install request that is deferred until the focused crate's feature metadata is loaded.
#[derive(Debug)]
pub struct PendingCargoRequest {
    pub intent: CargoIntent,
    pub crate_name: String,
}

/// What acting on the focused crate requires next, depending on the state of the crate's feature
/// metadata.
pub enum FeatureStep {
    /// Features are known and non-empty, open the feature picker.
    Pick(Box<FeatureSelector>),
    /// Features are known and there are none, run the cargo command directly.
    Run(Action),
    /// Features aren't loaded yet, load them, then decide again.
    AwaitMetadata { intent: CargoIntent, name: String },
    /// The action isn't valid for the focused crate's current state.
    Blocked(String),
}

/// Decides the next [`FeatureStep`] for an add/install of the focused crate, or `None` when nothing
/// is focused.
pub fn decide_feature_step(
    focused: Option<&Crate>,
    config: &Config,
    intent: CargoIntent,
    has_project: bool,
) -> Option<FeatureStep> {
    let cr = focused?;

    if intent == CargoIntent::Add {
        if !has_project {
            return Some(FeatureStep::Blocked(
                "No Cargo project found to add to".to_string(),
            ));
        }
        if cr.project_version.is_some() {
            return Some(FeatureStep::Blocked(format!(
                "{} is already a dependency of this project",
                cr.name
            )));
        }
    }

    let Some(features) = cr.features.as_deref() else {
        return Some(FeatureStep::AwaitMetadata {
            intent,
            name: cr.name.clone(),
        });
    };

    if features.is_empty() {
        return Some(FeatureStep::Run(intent.into_command(
            cr.name.clone(),
            cr.version.clone(),
            Vec::new(),
            false,
        )));
    }

    Some(FeatureStep::Pick(Box::new(FeatureSelector::new(
        config.clone(),
        cr.name.clone(),
        cr.version.clone(),
        intent,
        features,
        &cr.default_features,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn crate_with(features: Option<&[&str]>) -> Crate {
        Crate {
            id: "demo".into(),
            name: "demo".into(),
            version: "1.0.0".into(),
            features: features.map(|fs| fs.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn no_focus_yields_no_step() {
        assert!(decide_feature_step(None, &Config::default(), CargoIntent::Add, true).is_none());
    }

    #[test]
    fn unloaded_features_await_metadata() {
        let cr = crate_with(None);
        match decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Install, true) {
            Some(FeatureStep::AwaitMetadata { intent, name }) => {
                assert_eq!(intent, CargoIntent::Install);
                assert_eq!(name, "demo");
            }
            _ => panic!("expected AwaitMetadata"),
        }
    }

    #[test]
    fn no_features_runs_the_plain_command() {
        let cr = crate_with(Some(&[]));
        match decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Add, true) {
            Some(FeatureStep::Run(Action::Cargo(CargoCommand::Add {
                features,
                no_default_features,
                ..
            }))) => {
                assert!(features.is_empty());
                assert!(!no_default_features);
            }
            _ => panic!("expected Run(Add)"),
        }
    }

    #[test]
    fn known_features_open_the_picker() {
        let cr = crate_with(Some(&["derive", "std"]));
        assert!(matches!(
            decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Add, true),
            Some(FeatureStep::Pick(_))
        ));
    }

    #[test]
    fn add_without_a_project_is_blocked() {
        let cr = crate_with(Some(&[]));
        match decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Add, false) {
            Some(FeatureStep::Blocked(reason)) => assert!(reason.contains("No Cargo project")),
            _ => panic!("expected Blocked"),
        }
    }

    #[test]
    fn add_of_an_existing_dependency_is_blocked() {
        let mut cr = crate_with(Some(&[]));
        cr.project_version = Some("1.0".to_string());
        match decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Add, true) {
            Some(FeatureStep::Blocked(reason)) => assert!(reason.contains("already a dependency")),
            _ => panic!("expected Blocked"),
        }
    }

    #[test]
    fn install_ignores_the_project_and_duplicate_guards() {
        // Install has no project/duplicate guards: with no project and already present, it still runs.
        let mut cr = crate_with(Some(&[]));
        cr.project_version = Some("1.0".to_string());
        cr.installed_version = Some("1.0".to_string());
        assert!(matches!(
            decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Install, false),
            Some(FeatureStep::Run(_))
        ));
    }
}
