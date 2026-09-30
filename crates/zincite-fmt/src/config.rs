use std::num::NonZeroUsize;
use std::path::Path;

use ec4rs::Properties;
use zincite_fmt::{FormatOptions, IndentStyle, LineEnding};

pub struct Settings {
    pub format: FormatOptions,
    pub bom: bool,
}

pub fn override_key(argument: &std::ffi::OsStr) -> Option<&'static str> {
    match argument.to_str()? {
        "--indent-style" => Some("indent_style"),
        "--indent-size" => Some("indent_size"),
        "--tab-width" => Some("tab_width"),
        "--end-of-line" => Some("end_of_line"),
        "--max-line-length" => Some("max_line_length"),
        _ => None,
    }
}

pub fn set_override(properties: &mut Properties, key: &str, value: &str) -> Result<(), String> {
    let value = value.to_ascii_lowercase();
    validate(key, &value)?;
    if key == "indent_size" && value == "tab" {
        return Err("indent_size override requires a positive integer".into());
    }
    properties.insert_raw_for_key(key, value);
    Ok(())
}

pub fn resolve(path: Option<&Path>, overrides: &Properties) -> Result<Settings, String> {
    let mut properties = match path {
        Some(path) => {
            ec4rs::properties_of(path).map_err(|error| format!("EditorConfig: {error}"))?
        }
        None => Properties::new(),
    };
    // Values are case-insensitive. Reject explicitly empty supported properties
    // before normalizing unset for the core's indent/tab fallbacks, even when a
    // CLI option would override them.
    for (key, value) in properties.iter_mut() {
        if value.is_unset() {
            validate(key, "")?;
        }
        *value = value.to_lowercase();
        value.filter_unset_mut();
        if let Some(value) = value.into_option() {
            validate(key, value)?;
        }
    }
    for (key, value) in overrides.iter() {
        properties.insert_raw_for_key(key, value.clone());
    }
    properties.use_fallbacks();
    let get = |key| properties.get_raw_for_key(key).filter_unset().into_option();
    let mut format = FormatOptions::default();
    if let Some(value) = get("indent_style") {
        format.indent_style = match value {
            "tab" => IndentStyle::Tab,
            _ => IndentStyle::Space,
        };
    }
    if let Some(value) = get("tab_width") {
        format.tab_width = size("tab_width", value)?;
    }
    if let Some(value) = get("indent_size") {
        format.indent_size = if value == "tab" {
            format.tab_width
        } else {
            size("indent_size", value)?
        };
    }
    if let Some(value) = get("max_line_length") {
        format.max_line_length = if value == "off" {
            None
        } else {
            Some(size("max_line_length", value)?)
        };
    }
    if let Some(value) = get("end_of_line") {
        format.line_ending = match value {
            "crlf" => LineEnding::CrLf,
            "cr" => LineEnding::Cr,
            _ => LineEnding::Lf,
        };
    }
    if let Some(value) = get("insert_final_newline") {
        format.insert_final_newline = value == "true";
    }
    if let Some(value) = get("trim_trailing_whitespace") {
        format.trim_trailing_whitespace = value == "true";
    }
    Ok(Settings {
        format,
        bom: get("charset") == Some("utf-8-bom"),
    })
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    let valid = match key {
        "indent_style" => matches!(value, "space" | "tab"),
        "indent_size" if value == "tab" => true,
        "indent_size" | "tab_width" => return size(key, value).map(|_| ()),
        "max_line_length" if value == "off" => true,
        "max_line_length" => return size(key, value).map(|_| ()),
        "end_of_line" => matches!(value, "lf" | "crlf" | "cr"),
        "insert_final_newline" | "trim_trailing_whitespace" => matches!(value, "true" | "false"),
        "charset" => matches!(value, "utf-8" | "utf-8-bom"),
        _ => return Ok(()),
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "unsupported or invalid EditorConfig {key} = {value}"
        ))
    }
}

fn size(key: &str, value: &str) -> Result<NonZeroUsize, String> {
    value
        .parse::<usize>()
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| format!("invalid {key} = {value}; expected a positive integer"))
}
