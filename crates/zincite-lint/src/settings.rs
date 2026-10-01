//! Explicit lint settings; no filesystem or environment lookup.
//!
//! Personal presets are flat. For example, this preset combines families,
//! exclusions and options without enabling unavailable parameterized rules:
//! ```toml
//! [lint]
//! select = ["preset:personal"]
//! [lint.presets.personal]
//! select = ["family:style", "family:performance"]
//! ignore = ["expensive-comprehension", "decision-variable-operator"]
//! [lint.presets.personal.options.suspicious-shadowing]
//! ignore-names = ["i", "j"]
//! [lint.presets.personal.options.expensive-comprehension]
//! max-candidates = 50000
//! [lint.options.expensive-comprehension]
//! max-candidates = 25000
//! ```
//! Options merge defaults, the selected personal preset, then root overrides.
//! All definitions are validated, including disabled rules and unused presets.

use crate::rules::expand_selectors;
use crate::{LintOptions, Rule, RuleFamily};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

/// Effective concrete rule parameters, independent of diagnostic selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleOptions {
    /// Exact binding names to omit from suspicious-shadowing; default empty.
    pub shadowing_ignore_names: Vec<String>,
    /// Warn for a known candidate upper bound above this value; default 1000000.
    /// Symbolic expansion advice does not use this threshold.
    pub comprehension_max_candidates: NonZeroU64,
}
impl Default for RuleOptions {
    fn default() -> Self {
        Self {
            shadowing_ignore_names: Vec::new(),
            comprehension_max_candidates: NonZeroU64::new(1_000_000).unwrap(),
        }
    }
}

/// Optional overrides; an explicit empty name list clears preset names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleOptionOverrides {
    pub shadowing_ignore_names: Option<Vec<String>>,
    pub comprehension_max_candidates: Option<NonZeroU64>,
}
impl RuleOptionOverrides {
    fn apply(&self, options: &mut RuleOptions) {
        if let Some(names) = &self.shadowing_ignore_names {
            options.shadowing_ignore_names = names.clone();
        }
        if let Some(maximum) = self.comprehension_max_candidates {
            options.comprehension_max_candidates = maximum;
        }
    }
}

/// One named selection and its parameters. Members cannot name custom presets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonalPreset {
    pub select: Vec<String>,
    pub extend_select: Vec<String>,
    pub ignore: Vec<String>,
    pub options: RuleOptionOverrides,
}

/// Supported `[lint]` settings. Absence of select uses default; [] selects none.
/// Union extensions, then remove ignored rules. A personal preset applies its
/// own exclusions before the root's extensions and exclusions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LintSettings {
    pub select: Option<Vec<String>>,
    pub extend_select: Vec<String>,
    pub ignore: Vec<String>,
    pub presets: BTreeMap<String, PersonalPreset>,
    pub options: RuleOptionOverrides,
}

impl LintSettings {
    /// Parse supplied text; errors name settings, and the caller adds the path.
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let table = source
            .parse::<toml::Table>()
            .map_err(|error| format!("TOML: {error}"))?;
        for key in table.keys() {
            if key != "lint" {
                return Err(format!("unknown setting '{key}'"));
            }
        }
        let Some(lint) = table.get("lint") else {
            return Ok(Self::default());
        };
        let lint = lint.as_table().ok_or("lint: expected a table")?;
        let mut settings = Self::default();
        for (key, value) in lint {
            let path = format!("lint.{key}");
            match key.as_str() {
                "select" => settings.select = Some(strings(value, &path)?),
                "extend-select" => settings.extend_select = strings(value, &path)?,
                "ignore" => settings.ignore = strings(value, &path)?,
                "options" => settings.options = parse_options(value, &path)?,
                "presets" => {
                    for (name, value) in value.as_table().ok_or("lint.presets: expected a table")? {
                        let path = format!("lint.presets.{name}");
                        check_preset_name(name).map_err(|error| format!("{path}: {error}"))?;
                        let table = value
                            .as_table()
                            .ok_or_else(|| format!("{path}: expected a table"))?;
                        let mut preset = PersonalPreset::default();
                        if !table.contains_key("select") {
                            return Err(format!("{path}.select: required"));
                        }
                        for (key, value) in table {
                            let path = format!("{path}.{key}");
                            match key.as_str() {
                                "select" => preset.select = strings(value, &path)?,
                                "extend-select" => preset.extend_select = strings(value, &path)?,
                                "ignore" => preset.ignore = strings(value, &path)?,
                                "options" => preset.options = parse_options(value, &path)?,
                                _ => return Err(format!("unknown setting '{path}'")),
                            }
                        }
                        settings.presets.insert(name.clone(), preset);
                    }
                }
                _ => return Err(format!("unknown setting '{path}'")),
            }
        }
        settings.validate_presets()?;
        Ok(settings)
    }

    /// Resolve explicit settings without ambient lookup; unavailable selections
    /// remain inspectable and fail only when execution is requested.
    pub fn resolve(&self) -> Result<LintOptions, String> {
        self.validate_presets()?;
        let mut selected = None;
        let mut rules = match &self.select {
            Some(selectors) => self.expand(selectors, "lint.select", Some(&mut selected))?,
            None => LintOptions::default().rules,
        };
        union(
            &mut rules,
            self.expand(
                &self.extend_select,
                "lint.extend-select",
                Some(&mut selected),
            )?,
        );
        // Ignore is an exclusion, not another choice of a parameter bundle.
        let ignored = self.expand(&self.ignore, "lint.ignore", None)?;
        rules.retain(|rule| !ignored.contains(rule));
        let mut parameters = RuleOptions::default();
        if let Some(name) = selected {
            self.presets[&name].options.apply(&mut parameters);
        }
        self.options.apply(&mut parameters);
        Ok(LintOptions { rules, parameters })
    }

    /// Replace configured selection/exclusions with a CLI selection. Ordinary
    /// selectors retain the configured effective options. Selecting a personal
    /// preset instead uses that preset's options followed by root overrides;
    /// two personal parameter bundles are never composed.
    pub fn resolve_selection(&self, selection: &[String]) -> Result<LintOptions, String> {
        let configured = self.resolve()?;
        let mut selected = None;
        let rules = self.expand(selection, "--rules", Some(&mut selected))?;
        let mut parameters = configured.parameters;
        if let Some(name) = selected {
            parameters = RuleOptions::default();
            self.presets[&name].options.apply(&mut parameters);
            self.options.apply(&mut parameters);
        }
        Ok(LintOptions { rules, parameters })
    }

    fn validate_presets(&self) -> Result<(), String> {
        for (name, preset) in &self.presets {
            let path = format!("lint.presets.{name}");
            check_preset_name(name).map_err(|error| format!("{path}: {error}"))?;
            for (field, selectors) in [
                ("select", &preset.select),
                ("extend-select", &preset.extend_select),
                ("ignore", &preset.ignore),
            ] {
                // The built-in expander intentionally knows no personal presets.
                expand_selectors(selectors.iter().map(String::as_str)).map_err(|error| format!("{path}.{field}: {error}; personal presets cannot include other personal presets"))?;
            }
        }
        Ok(())
    }

    fn expand(
        &self,
        selectors: &[String],
        path: &str,
        mut selected: Option<&mut Option<String>>,
    ) -> Result<Vec<Rule>, String> {
        let mut rules = Vec::new();
        for selector in selectors {
            let name = selector.strip_prefix("preset:").unwrap_or(selector);
            if let Some(preset) = self.presets.get(name) {
                if let Some(selected) = selected.as_deref_mut() {
                    if selected.as_ref().is_some_and(|previous| previous != name) {
                        return Err(format!("{path}: more than one personal preset selected"));
                    }
                    *selected = Some(name.to_owned());
                }
                let mut members = expand_selectors(preset.select.iter().map(String::as_str))?;
                union(
                    &mut members,
                    expand_selectors(preset.extend_select.iter().map(String::as_str))?,
                );
                let ignored = expand_selectors(preset.ignore.iter().map(String::as_str))?;
                members.retain(|rule| !ignored.contains(rule));
                union(&mut rules, members);
            } else {
                union(
                    &mut rules,
                    expand_selectors([selector.as_str()])
                        .map_err(|error| format!("{path}: {error}"))?,
                );
            }
        }
        Ok(rules)
    }
}

fn check_preset_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.contains(':')
        || ["default", "thesis", "all"].contains(&name)
        || Rule::from_id(name).is_some()
        || RuleFamily::from_name(name).is_some()
    {
        Err(format!("reserved or invalid personal preset name '{name}'"))
    } else {
        Ok(())
    }
}
fn union(rules: &mut Vec<Rule>, additions: Vec<Rule>) {
    for rule in additions {
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }
}
fn strings(value: &toml::Value, path: &str) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{path}: expected an array of strings"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{path}[{index}]: expected a string"))
        })
        .collect()
}
fn parse_options(value: &toml::Value, path: &str) -> Result<RuleOptionOverrides, String> {
    let table = value
        .as_table()
        .ok_or_else(|| format!("{path}: expected a table"))?;
    let mut options = RuleOptionOverrides::default();
    for (rule, value) in table {
        let path = format!("{path}.{rule}");
        if !["suspicious-shadowing", "expensive-comprehension"].contains(&rule.as_str()) {
            return Err(format!("unknown setting '{path}'"));
        }
        let table = value
            .as_table()
            .ok_or_else(|| format!("{path}: expected a table"))?;
        for (key, value) in table {
            let path = format!("{path}.{key}");
            match (rule.as_str(), key.as_str()) {
                ("suspicious-shadowing", "ignore-names") => {
                    options.shadowing_ignore_names = Some(strings(value, &path)?)
                }
                ("expensive-comprehension", "max-candidates") => {
                    let maximum =
                        value
                            .as_integer()
                            .filter(|number| *number > 0)
                            .ok_or_else(|| {
                                format!("{path}: expected an integer in 1..=9223372036854775807")
                            })?;
                    options.comprehension_max_candidates = NonZeroU64::new(maximum as u64);
                }
                _ => return Err(format!("unknown setting '{path}'")),
            }
        }
    }
    Ok(options)
}
