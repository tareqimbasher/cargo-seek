use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::action::Action;
use crate::cargo::CargoCommand;
use crate::components::ux::{KeyOutcome, MultiSelect, MultiSelectItem};
use crate::config::Config;

/// Which cargo action the picker dispatches once features are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureIntent {
    Add,
    Install,
}

/// A multi-select checklist of a crate's features for the user to select from when adding or
/// installing a crate.
pub struct FeatureSelector {
    crate_name: String,
    version: String,
    intent: FeatureIntent,
    default_features: Vec<String>,
    selector: MultiSelect<String>,
}

impl FeatureSelector {
    pub fn new(
        config: Config,
        crate_name: String,
        version: String,
        intent: FeatureIntent,
        features: &[String],
        default_features: &[String],
    ) -> Self {
        let items = features
            .iter()
            .map(|name| {
                let is_default = default_features.iter().any(|d| d == name);
                let label: Line<'static> = if is_default {
                    name.clone().bold().into()
                } else {
                    name.clone().into()
                };
                // Defaults start checked so confirming straight away matches a plain add.
                MultiSelectItem::new(name.clone(), label, is_default)
            })
            .collect();

        let verb = match intent {
            FeatureIntent::Add => "Add",
            FeatureIntent::Install => "Install",
        };

        Self {
            crate_name: crate_name.clone(),
            version,
            intent,
            default_features: default_features.to_vec(),
            selector: MultiSelect::new(config, format!(" {verb} {crate_name} — features "), items),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyOutcome<Action> {
        match self.selector.handle_key(key) {
            KeyOutcome::Pending => KeyOutcome::Pending,
            KeyOutcome::Cancelled => KeyOutcome::Cancelled,
            KeyOutcome::Submitted(checked) => KeyOutcome::Submitted(self.command(&checked)),
        }
    }

    /// Builds the cargo command for the chosen feature set.
    ///
    /// When every default feature is still checked, cargo enables them implicitly, so only the
    /// extra (non-default) selections are passed. If the user unchecked any default, the defaults
    /// are turned off (`--no-default-features`) and the full kept set is passed explicitly.
    fn command(&self, checked: &[String]) -> Action {
        let no_default_features = self.default_features.iter().any(|d| !checked.contains(d));

        let features: Vec<String> = checked
            .iter()
            .filter(|name| no_default_features || !self.default_features.contains(name))
            .cloned()
            .collect();

        let name = self.crate_name.clone();
        let version = self.version.clone();

        let command = match self.intent {
            FeatureIntent::Add => CargoCommand::Add {
                name,
                version,
                features,
                no_default_features,
            },
            FeatureIntent::Install => CargoCommand::Install {
                name,
                version,
                features,
                no_default_features,
            },
        };
        Action::Cargo(command)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.selector.draw(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::{FeatureIntent, FeatureSelector};
    use crate::action::Action;
    use crate::cargo::CargoCommand;
    use crate::components::ux::KeyOutcome;
    use crate::config::Config;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use pretty_assertions::assert_eq;

    fn selector(features: &[&str], defaults: &[&str]) -> FeatureSelector {
        let features: Vec<String> = features.iter().map(|s| s.to_string()).collect();
        let defaults: Vec<String> = defaults.iter().map(|s| s.to_string()).collect();
        FeatureSelector::new(
            Config::default(),
            "demo".into(),
            "1.0.0".into(),
            FeatureIntent::Add,
            &features,
            &defaults,
        )
    }

    fn press(sel: &mut FeatureSelector, code: KeyCode) -> KeyOutcome<Action> {
        sel.handle_key(KeyEvent::new(code, KeyModifiers::empty()))
    }

    /// Toggles the checkbox at `index` by walking the selection there from the top.
    fn toggle(sel: &mut FeatureSelector, index: usize) {
        for _ in 0..index {
            press(sel, KeyCode::Down);
        }
        press(sel, KeyCode::Char(' '));
    }

    /// Confirms the selection and returns the resulting add command's feature args.
    fn add_args(sel: &mut FeatureSelector) -> (Vec<String>, bool) {
        match press(sel, KeyCode::Enter) {
            KeyOutcome::Submitted(Action::Cargo(CargoCommand::Add {
                features,
                no_default_features,
                ..
            })) => (features, no_default_features),
            other => panic!("expected an Add command, got {other:?}"),
        }
    }

    #[test]
    fn keeping_defaults_passes_no_extra_features() {
        let mut sel = selector(&["derive", "std", "env"], &["derive", "std"]);
        let (features, no_default_features) = add_args(&mut sel);

        // Defaults stay enabled implicitly, so nothing is passed and they aren't disabled.
        assert!(features.is_empty());
        assert!(!no_default_features);
    }

    #[test]
    fn enabling_a_non_default_passes_only_that_feature() {
        let mut sel = selector(&["derive", "std", "env"], &["derive", "std"]);
        toggle(&mut sel, 2); // enable "env"
        let (features, no_default_features) = add_args(&mut sel);

        assert_eq!(features, vec!["env".to_string()]);
        assert!(!no_default_features);
    }

    #[test]
    fn unchecking_a_default_disables_defaults_and_passes_the_kept_set() {
        let mut sel = selector(&["derive", "std", "env"], &["derive", "std"]);
        toggle(&mut sel, 0); // disable the "derive" default
        let (features, no_default_features) = add_args(&mut sel);

        // With a default turned off, defaults are disabled and the surviving selection is explicit.
        assert!(no_default_features);
        assert_eq!(features, vec!["std".to_string()]);
    }

    #[test]
    fn crate_without_defaults_passes_the_selection_as_is() {
        let mut sel = selector(&["a", "b"], &[]);
        toggle(&mut sel, 0); // enable "a"
        let (features, no_default_features) = add_args(&mut sel);

        assert_eq!(features, vec!["a".to_string()]);
        assert!(!no_default_features);
    }
}
