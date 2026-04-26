//! Maps style names to factories. Factories return a fresh
//! [`Style`] instance each time, so `--shuffle` can rotate through
//! styles without leaking per-style internal state across rotations.

use crate::Style;
use crate::styles;

pub type StyleFactory = fn() -> Box<dyn Style>;

pub struct Registry {
    pairs: &'static [(&'static str, StyleFactory)],
}

impl Registry {
    pub fn new() -> Self {
        // Pinned ordering mirrors the plan's style table; --list
        // emits in this order for predictable output.
        Self {
            pairs: &[
                ("logo-bounce", || Box::new(styles::logo_bounce::LogoBounce::new())),
                ("matrix",      || Box::new(styles::matrix::Matrix::new())),
                ("plasma",      || Box::new(styles::plasma::Plasma::new())),
                ("starfield",   || Box::new(styles::starfield::Starfield::new())),
                ("conway",      || Box::new(styles::conway::Conway::new())),
                ("tunnel",      || Box::new(styles::tunnel::Tunnel::new())),
                ("waves",       || Box::new(styles::waves::Waves::new())),
                ("mandala",     || Box::new(styles::mandala::Mandala::new())),
            ],
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.pairs.iter().map(|(k, _)| *k)
    }

    pub fn get(&self, key: &str) -> Option<StyleFactory> {
        self.pairs.iter().find(|(k, _)| *k == key).map(|(_, f)| *f)
    }

    pub fn instantiate(&self, key: &str) -> Option<Box<dyn Style>> {
        self.get(key).map(|f| f())
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_eight_styles() {
        assert_eq!(Registry::new().len(), 8);
    }

    #[test]
    fn registry_keys_match_plan() {
        let keys: Vec<&str> = Registry::new().keys().collect();
        assert_eq!(
            keys,
            vec![
                "logo-bounce",
                "matrix",
                "plasma",
                "starfield",
                "conway",
                "tunnel",
                "waves",
                "mandala",
            ]
        );
    }

    #[test]
    fn instantiate_unknown_returns_none() {
        assert!(Registry::new().instantiate("nope").is_none());
    }

    #[test]
    fn each_factory_constructs_a_distinct_style() {
        let r = Registry::new();
        for k in r.keys() {
            let s = r.instantiate(k).unwrap();
            assert_eq!(s.name(), k);
            assert!(!s.title().is_empty());
        }
    }
}
