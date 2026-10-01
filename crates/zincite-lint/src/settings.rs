//! Explicit lint selection settings. Parsing and resolution do not read files
//! or consult the environment; the command owns configuration discovery.

use crate::LintOptions;
use crate::rules::expand_selectors;

/// The supported `[lint]` fields in `zincite.toml`.
///
/// An absent `select` uses the default preset; an empty list selects no rules.
/// Expand `select`, union `extend-select`, then remove `ignore`. Selectors are
/// exact IDs, `family:NAME`, built-in `preset:NAME`, or legacy preset names.
/// Custom presets, rule parameters and fix restrictions are not supported yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LintSettings {
    pub select: Option<Vec<String>>,
    pub extend_select: Vec<String>,
    pub ignore: Vec<String>,
}

impl LintSettings {
    /// Parse supplied TOML text, rejecting unknown tables/keys and wrong types.
    /// Errors name the setting; a file-reading caller adds its settings path.
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
            if !matches!(key.as_str(), "select" | "extend-select" | "ignore") {
                return Err(format!("unknown setting 'lint.{key}'"));
            }
            let values = value
                .as_array()
                .ok_or_else(|| format!("lint.{key}: expected an array of selectors"))?;
            let selectors = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("lint.{key}[{index}]: expected a string selector"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            match key.as_str() {
                "select" => settings.select = Some(selectors),
                "extend-select" => settings.extend_select = selectors,
                "ignore" => settings.ignore = selectors,
                _ => unreachable!(),
            }
        }
        Ok(settings)
    }

    /// Resolve supplied settings without filesystem lookup. Availability is
    /// checked before execution through `LintOptions::check_available`.
    pub fn resolve(&self) -> Result<LintOptions, String> {
        let mut rules = match &self.select {
            Some(selectors) => expand_selectors(selectors.iter().map(String::as_str))
                .map_err(|error| format!("lint.select: {error}"))?,
            None => LintOptions::default().rules,
        };
        let extensions = expand_selectors(self.extend_select.iter().map(String::as_str))
            .map_err(|error| format!("lint.extend-select: {error}"))?;
        let ignored = expand_selectors(self.ignore.iter().map(String::as_str))
            .map_err(|error| format!("lint.ignore: {error}"))?;
        for rule in extensions {
            if !rules.contains(&rule) {
                rules.push(rule);
            }
        }
        rules.retain(|rule| !ignored.contains(rule));
        Ok(LintOptions { rules })
    }
}
