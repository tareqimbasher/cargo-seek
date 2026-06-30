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
}

/// Decides the next [`FeatureStep`] for an add/install of the focused crate, or `None` when nothing
/// is focused.
pub fn decide_feature_step(
    focused: Option<&Crate>,
    config: &Config,
    intent: CargoIntent,
) -> Option<FeatureStep> {
    let cr = focused?;

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
        assert!(decide_feature_step(None, &Config::default(), CargoIntent::Add).is_none());
    }

    #[test]
    fn unloaded_features_await_metadata() {
        let cr = crate_with(None);
        match decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Install) {
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
        match decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Add) {
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
            decide_feature_step(Some(&cr), &Config::default(), CargoIntent::Add),
            Some(FeatureStep::Pick(_))
        ));
    }
}
