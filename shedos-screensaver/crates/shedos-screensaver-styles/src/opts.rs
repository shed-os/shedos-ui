use std::collections::HashMap;
use std::fmt;

/// Typed value for a per-style option.
#[derive(Debug, Clone, PartialEq)]
pub enum OptVal {
    Bool(bool),
    UInt(u32),
    Float(f32),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptType {
    Bool,
    UInt,
    Float,
    Enum, // string-valued, validated against an allowed set
    String,
}

impl OptType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::UInt => "u32",
            Self::Float => "f32",
            Self::Enum => "enum",
            Self::String => "string",
        }
    }
}

/// One option in a style's schema.
pub struct OptionDoc {
    pub key: &'static str,
    pub ty: OptType,
    pub default: OptVal,
    /// Human-readable describing what the option does and
    /// (for numeric options) what the valid range is.
    pub desc: &'static str,
    /// Validator. Returns Ok(()) if the value is acceptable.
    pub validate: fn(&OptVal) -> Result<(), String>,
}

pub struct OptionSchema {
    pub options: &'static [OptionDoc],
}

impl OptionSchema {
    pub fn empty() -> &'static Self {
        static EMPTY: OptionSchema = OptionSchema { options: &[] };
        &EMPTY
    }

    pub fn lookup(&self, key: &str) -> Option<&OptionDoc> {
        self.options.iter().find(|o| o.key == key)
    }
}

/// Resolved per-style options: defaults overridden by config file
/// values, then by `--style-opt KEY=VAL` CLI flags.
#[derive(Debug, Clone, Default)]
pub struct StyleOpts {
    values: HashMap<String, OptVal>,
}

impl StyleOpts {
    /// Build a StyleOpts initialized with the schema's defaults.
    pub fn from_defaults(schema: &OptionSchema) -> Self {
        let mut values = HashMap::new();
        for opt in schema.options {
            values.insert(opt.key.to_string(), opt.default.clone());
        }
        Self { values }
    }

    /// Override one option from a `KEY=VAL` string. The value is
    /// parsed according to the schema's declared type and validated
    /// via the schema's validator.
    pub fn set(&mut self, schema: &OptionSchema, kv: &str) -> Result<(), OptionSetError> {
        let (k, v) = kv.split_once('=').ok_or_else(|| OptionSetError::Syntax(kv.to_string()))?;
        let opt = schema.lookup(k).ok_or_else(|| OptionSetError::UnknownKey(k.to_string()))?;
        let parsed = match opt.ty {
            OptType::Bool => match v {
                "true" | "yes" | "on" | "1" => OptVal::Bool(true),
                "false" | "no" | "off" | "0" => OptVal::Bool(false),
                other => return Err(OptionSetError::Parse {
                    key: k.to_string(),
                    expected: "bool",
                    given: other.to_string(),
                }),
            },
            OptType::UInt => v.parse::<u32>().map(OptVal::UInt).map_err(|_| {
                OptionSetError::Parse {
                    key: k.to_string(),
                    expected: "u32",
                    given: v.to_string(),
                }
            })?,
            OptType::Float => v.parse::<f32>().map(OptVal::Float).map_err(|_| {
                OptionSetError::Parse {
                    key: k.to_string(),
                    expected: "f32",
                    given: v.to_string(),
                }
            })?,
            OptType::Enum | OptType::String => OptVal::String(v.to_string()),
        };
        if let Err(reason) = (opt.validate)(&parsed) {
            return Err(OptionSetError::Range {
                key: k.to_string(),
                given: v.to_string(),
                reason,
            });
        }
        self.values.insert(k.to_string(), parsed);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&OptVal> {
        self.values.get(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.values.get(key) {
            Some(OptVal::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn get_u32(&self, key: &str) -> Option<u32> {
        match self.values.get(key) {
            Some(OptVal::UInt(u)) => Some(*u),
            _ => None,
        }
    }

    pub fn get_f32(&self, key: &str) -> Option<f32> {
        match self.values.get(key) {
            Some(OptVal::Float(f)) => Some(*f),
            _ => None,
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.values.get(key) {
            Some(OptVal::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OptionSetError {
    Syntax(String),
    UnknownKey(String),
    Parse { key: String, expected: &'static str, given: String },
    Range { key: String, given: String, reason: String },
}

impl fmt::Display for OptionSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "invalid --style-opt '{s}': expected KEY=VAL"),
            Self::UnknownKey(k) => write!(f, "unknown style option '{k}'"),
            Self::Parse { key, expected, given } => {
                write!(f, "option '{key}' must be {expected}; got '{given}'")
            }
            Self::Range { key, given, reason } => {
                write!(f, "option '{key}' = '{given}' rejected: {reason}")
            }
        }
    }
}

impl std::error::Error for OptionSetError {}

/// Helper: range-validator for floats.
pub fn validate_f32_range(min: f32, max: f32) -> impl Fn(&OptVal) -> Result<(), String> {
    move |v: &OptVal| match v {
        OptVal::Float(f) if (min..=max).contains(f) => Ok(()),
        OptVal::Float(f) => Err(format!("must be in {min}..={max} (got {f})")),
        _ => Err("expected float".to_string()),
    }
}

/// Helper: range-validator for unsigned ints.
pub fn validate_u32_range(min: u32, max: u32) -> impl Fn(&OptVal) -> Result<(), String> {
    move |v: &OptVal| match v {
        OptVal::UInt(u) if (min..=max).contains(u) => Ok(()),
        OptVal::UInt(u) => Err(format!("must be in {min}..={max} (got {u})")),
        _ => Err("expected u32".to_string()),
    }
}

/// Helper: enum validator (one of a fixed set of string values).
pub fn validate_enum(allowed: &'static [&'static str]) -> impl Fn(&OptVal) -> Result<(), String> {
    move |v: &OptVal| match v {
        OptVal::String(s) if allowed.contains(&s.as_str()) => Ok(()),
        OptVal::String(s) => Err(format!("must be one of {allowed:?} (got '{s}')")),
        _ => Err("expected string".to_string()),
    }
}

/// Helper: always-OK validator.
pub fn validate_any(_: &OptVal) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static MATRIX_GLYPHS: &[&str] = &["katakana", "ascii", "hex", "brand"];

    fn dummy_schema() -> &'static OptionSchema {
        // Avoid relying on real style schemas in opts unit tests; use a
        // hand-rolled one to exercise the parsing/validation surface.
        // Statics holding closures are awkward; use a fn pointer to a
        // small helper that closes over only static data.
        fn dens_v(v: &OptVal) -> Result<(), String> {
            match v {
                OptVal::Float(f) if (0.0..=1.0).contains(f) => Ok(()),
                OptVal::Float(_) => Err("must be in 0.0..=1.0".into()),
                _ => Err("expected float".into()),
            }
        }
        fn trail_v(v: &OptVal) -> Result<(), String> {
            match v {
                OptVal::UInt(u) if (1..=100).contains(u) => Ok(()),
                OptVal::UInt(_) => Err("must be in 1..=100".into()),
                _ => Err("expected u32".into()),
            }
        }
        fn glyph_v(v: &OptVal) -> Result<(), String> {
            match v {
                OptVal::String(s) if MATRIX_GLYPHS.contains(&s.as_str()) => Ok(()),
                OptVal::String(_) => Err("invalid glyph set".into()),
                _ => Err("expected string".into()),
            }
        }
        static SCHEMA: OptionSchema = OptionSchema {
            options: &[
                OptionDoc { key: "density", ty: OptType::Float, default: OptVal::Float(0.5), desc: "", validate: dens_v },
                OptionDoc { key: "trail_length", ty: OptType::UInt, default: OptVal::UInt(20), desc: "", validate: trail_v },
                OptionDoc { key: "glyphs", ty: OptType::Enum, default: OptVal::String(String::new()), desc: "", validate: glyph_v },
            ],
        };
        // String default needed at runtime since `String::new()` isn't const-friendly
        // in stable for all cases; we initialize it on first lookup if needed.
        // Simpler: leave default as empty and treat it as opaque for tests.
        &SCHEMA
    }

    #[test]
    fn from_defaults_populates_all_options() {
        let opts = StyleOpts::from_defaults(dummy_schema());
        assert_eq!(opts.get_f32("density"), Some(0.5));
        assert_eq!(opts.get_u32("trail_length"), Some(20));
    }

    #[test]
    fn set_valid_values_succeeds() {
        let schema = dummy_schema();
        let mut opts = StyleOpts::from_defaults(schema);
        opts.set(schema, "density=0.7").unwrap();
        opts.set(schema, "trail_length=50").unwrap();
        opts.set(schema, "glyphs=katakana").unwrap();
        assert_eq!(opts.get_f32("density"), Some(0.7));
        assert_eq!(opts.get_u32("trail_length"), Some(50));
        assert_eq!(opts.get_str("glyphs"), Some("katakana"));
    }

    #[test]
    fn set_out_of_range_errors() {
        let schema = dummy_schema();
        let mut opts = StyleOpts::from_defaults(schema);
        let err = opts.set(schema, "density=99").unwrap_err();
        assert!(matches!(err, OptionSetError::Range { .. }));
    }

    #[test]
    fn set_wrong_type_errors() {
        let schema = dummy_schema();
        let mut opts = StyleOpts::from_defaults(schema);
        let err = opts.set(schema, "trail_length=not-a-number").unwrap_err();
        assert!(matches!(err, OptionSetError::Parse { .. }));
    }

    #[test]
    fn set_unknown_key_errors() {
        let schema = dummy_schema();
        let mut opts = StyleOpts::from_defaults(schema);
        let err = opts.set(schema, "frobnicate=true").unwrap_err();
        assert!(matches!(err, OptionSetError::UnknownKey(_)));
    }

    #[test]
    fn set_bad_syntax_errors() {
        let schema = dummy_schema();
        let mut opts = StyleOpts::from_defaults(schema);
        let err = opts.set(schema, "no-equals-sign").unwrap_err();
        assert!(matches!(err, OptionSetError::Syntax(_)));
    }

    #[test]
    fn enum_validator_rejects_unknown() {
        let schema = dummy_schema();
        let mut opts = StyleOpts::from_defaults(schema);
        let err = opts.set(schema, "glyphs=cyrillic").unwrap_err();
        assert!(matches!(err, OptionSetError::Range { .. }));
    }
}
