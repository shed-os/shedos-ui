//! Fluent-rs wrapper for shedos-screensaver.
//!
//! Catalogs at `/usr/share/locale/<lang>/LC_MESSAGES/shedos-screensaver.ftl`,
//! with en-US embedded at compile time as a hard fallback. Locale
//! resolution: explicit `--locale` → `$LC_ALL` → `$LC_MESSAGES` →
//! `$LANG` → `en-US`. Missing keys fall back to en-US, then to the
//! bare key.

pub use fluent::{FluentArgs, FluentValue};
use fluent::FluentResource;
// Use the concurrent FluentBundle (Send + Sync) so the global
// OnceLock<RwLock<I18n>> compiles. The single-threaded default bundle
// wraps `IntlLangMemoizer { RefCell<TypeMap> }` which is !Send.
use fluent_bundle::concurrent::FluentBundle;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use unic_langid::{langid, LanguageIdentifier};

const EN_US_FTL: &str = include_str!("../../../i18n/en-US.ftl");
const SYSTEM_LOCALE_DIR: &str = "/usr/share/locale";

static BUNDLE: OnceLock<RwLock<I18n>> = OnceLock::new();

pub struct I18n {
    primary: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

impl I18n {
    /// Initialize the global bundle with the resolved locale.
    /// Calling this more than once is a no-op (returns Ok with the
    /// already-resolved locale).
    pub fn init(explicit: Option<&str>) -> Result<LanguageIdentifier, I18nError> {
        let lang = resolve_locale(explicit);
        let primary = build_bundle_for(&lang)?;
        // Fallback uses the compile-time embedded en-US, not the
        // on-disk one. New keys then resolve even if the installed
        // shedos-screensaver.ftl is older (binary-then-package
        // upgrade window).
        let fallback = build_embedded_bundle()?;
        let i18n = I18n { primary, fallback };
        let _ = BUNDLE.set(RwLock::new(i18n));
        Ok(lang)
    }

    /// Lookup `key`; format with `args`. Falls back through:
    /// primary bundle → en-US embedded → bare key.
    pub fn t(key: &str, args: Option<&FluentArgs>) -> String {
        let cell = match BUNDLE.get() {
            Some(c) => c,
            None => return key.to_string(),
        };
        let g = match cell.read() {
            Ok(g) => g,
            Err(_) => return key.to_string(),
        };
        if let Some(s) = format_in(&g.primary, key, args) {
            return s;
        }
        if let Some(s) = format_in(&g.fallback, key, args) {
            return s;
        }
        key.to_string()
    }
}

fn format_in(
    bundle: &FluentBundle<FluentResource>,
    key: &str,
    args: Option<&FluentArgs>,
) -> Option<String> {
    let msg = bundle.get_message(key)?;
    let pattern = msg.value()?;
    let mut errors = Vec::new();
    let s = bundle.format_pattern(pattern, args, &mut errors);
    if !errors.is_empty() {
        // Format errors (missing args etc.); partial result is returned.
    }
    Some(s.into_owned())
}

fn build_bundle_for(lang: &LanguageIdentifier) -> Result<FluentBundle<FluentResource>, I18nError> {
    let mut bundle = FluentBundle::new_concurrent(vec![lang.clone()]);
    bundle.set_use_isolating(false);
    let source = load_source_for(lang).unwrap_or_else(|| EN_US_FTL.to_string());
    let res = FluentResource::try_new(source).map_err(|(_, errs)| I18nError::Parse(format!("{errs:?}")))?;
    bundle.add_resource(res).map_err(|errs| I18nError::AddResource(format!("{errs:?}")))?;
    Ok(bundle)
}

fn build_embedded_bundle() -> Result<FluentBundle<FluentResource>, I18nError> {
    let mut bundle = FluentBundle::new_concurrent(vec![langid!("en-US")]);
    bundle.set_use_isolating(false);
    let res = FluentResource::try_new(EN_US_FTL.to_string())
        .map_err(|(_, errs)| I18nError::Parse(format!("{errs:?}")))?;
    bundle
        .add_resource(res)
        .map_err(|errs| I18nError::AddResource(format!("{errs:?}")))?;
    Ok(bundle)
}

fn load_source_for(lang: &LanguageIdentifier) -> Option<String> {
    let path = locale_path(lang);
    std::fs::read_to_string(path).ok()
}

fn locale_path(lang: &LanguageIdentifier) -> PathBuf {
    PathBuf::from(SYSTEM_LOCALE_DIR)
        .join(lang.language.as_str())
        .join("LC_MESSAGES")
        .join("shedos-screensaver.ftl")
}

fn resolve_locale(explicit: Option<&str>) -> LanguageIdentifier {
    let candidates: Vec<String> = explicit
        .map(str::to_string)
        .into_iter()
        .chain(std::env::var("LC_ALL").ok())
        .chain(std::env::var("LC_MESSAGES").ok())
        .chain(std::env::var("LANG").ok())
        .collect();
    for raw in candidates {
        if let Some(parsed) = parse_locale(&raw) {
            return parsed;
        }
    }
    langid!("en-US")
}

fn parse_locale(raw: &str) -> Option<LanguageIdentifier> {
    // Strip codeset (`en_US.UTF-8` → `en-US`) and treat `_` as `-`.
    let core = raw.split('.').next().unwrap_or("");
    let normalized = core.replace('_', "-");
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("c") || normalized.eq_ignore_ascii_case("posix") {
        return None;
    }
    normalized.parse().ok()
}

/// Convenience helper for ad-hoc lookups without args.
pub fn t(key: &str) -> String {
    I18n::t(key, None)
}

/// Convenience helper for lookups with args.
pub fn t_args(key: &str, args: &[(&str, FluentValue<'_>)]) -> String {
    let mut a = FluentArgs::new();
    for (k, v) in args {
        a.set(*k, v.clone());
    }
    I18n::t(key, Some(&a))
}

/// Convenience helper for all-string args. Saves call sites from
/// constructing `FluentValue::from(...)` repeatedly.
pub fn t_str(key: &str, args: &[(&str, &str)]) -> String {
    let mut a = FluentArgs::new();
    for (k, v) in args {
        a.set(*k, FluentValue::from(*v));
    }
    I18n::t(key, Some(&a))
}

#[derive(Debug)]
pub enum I18nError {
    Parse(String),
    AddResource(String),
}

impl std::fmt::Display for I18nError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "fluent parse error: {s}"),
            Self::AddResource(s) => write!(f, "fluent add-resource error: {s}"),
        }
    }
}

impl std::error::Error for I18nError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn parse_locale_strips_codeset() {
        assert_eq!(
            parse_locale("en_US.UTF-8"),
            Some(langid!("en-US"))
        );
    }

    #[test]
    fn parse_locale_normalizes_separator() {
        assert_eq!(parse_locale("en_GB"), Some(langid!("en-GB")));
    }

    #[test]
    fn parse_locale_rejects_c_and_posix() {
        assert_eq!(parse_locale("C"), None);
        assert_eq!(parse_locale("POSIX"), None);
        assert_eq!(parse_locale("c.UTF-8"), None);
    }

    #[test]
    fn parse_locale_rejects_empty() {
        assert_eq!(parse_locale(""), None);
        assert_eq!(parse_locale(".UTF-8"), None);
    }

    #[test]
    fn explicit_locale_wins() {
        // SAFETY: env mutation is process-global; the test is single-threaded
        // by virtue of being in this module. Future tests in this module
        // that touch env should be fenced by this test if added.
        unsafe {
            env::set_var("LANG", "fr_FR.UTF-8");
        }
        let l = resolve_locale(Some("ja-JP"));
        assert_eq!(l, langid!("ja-JP"));
        unsafe {
            env::remove_var("LANG");
        }
    }

    #[test]
    fn embedded_en_us_loads_and_formats() {
        // Seed the global with en-US.
        let _ = I18n::init(Some("en-US")).unwrap();
        // The embedded catalog must contain `app-name` (a sanity key).
        let s = t("app-name");
        assert!(!s.is_empty());
        assert_ne!(s, "app-name", "key returned bare — catalog missing 'app-name'");
    }

    #[test]
    fn missing_key_falls_back_to_bare() {
        let _ = I18n::init(Some("en-US")).unwrap();
        let s = t("a-key-that-definitely-does-not-exist");
        assert_eq!(s, "a-key-that-definitely-does-not-exist");
    }
}
