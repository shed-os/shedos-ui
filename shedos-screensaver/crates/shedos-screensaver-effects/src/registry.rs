//! Registry of effects. Maps a name to a factory that returns a
//! fresh `Box<dyn Effect>`. Adding an effect = importing it in the
//! `effects` module and adding one row here.

use crate::effects;
use crate::Effect;

pub type EffectFactory = fn() -> Box<dyn Effect>;

pub struct Registry {
    pairs: &'static [(&'static str, EffectFactory)],
}

impl Registry {
    pub fn new() -> Self {
        Self {
            pairs: &[
                ("rain",          || Box::new(effects::rain::Rain::new())),
                ("decrypt",       || Box::new(effects::decrypt::Decrypt::new())),
                ("print",         || Box::new(effects::print::Print::new())),
                ("scattered",     || Box::new(effects::scattered::Scattered::new())),
                ("wipe",          || Box::new(effects::wipe::Wipe::new())),
                ("slide",         || Box::new(effects::slide::Slide::new())),
                ("expand",        || Box::new(effects::expand::Expand::new())),
                ("crumble",       || Box::new(effects::crumble::Crumble::new())),
                ("spotlights",    || Box::new(effects::spotlights::Spotlights::new())),
                ("burn",          || Box::new(effects::burn::Burn::new())),
                ("colorshift",    || Box::new(effects::colorshift::Colorshift::new())),
                ("glitch",        || Box::new(effects::glitch::Glitch::new())),
                ("quantum",       || Box::new(effects::quantum::Quantum::new())),
                ("synthgrid",     || Box::new(effects::synthgrid::Synthgrid::new())),
                ("matrix-rain",   || Box::new(effects::matrix_rain::MatrixRain::new())),
                ("hologram",      || Box::new(effects::hologram::Hologram::new())),
                ("neon-trace",    || Box::new(effects::neon_trace::NeonTrace::new())),
                ("blackhole",     || Box::new(effects::blackhole::Blackhole::new())),
                ("shockwave",     || Box::new(effects::shockwave::Shockwave::new())),
                ("liquid-fill",   || Box::new(effects::liquid_fill::LiquidFill::new())),
                ("constellation", || Box::new(effects::constellation::Constellation::new())),
                ("interlace",     || Box::new(effects::interlace::Interlace::new())),
                ("thermal",       || Box::new(effects::thermal::Thermal::new())),
                ("data-stream",   || Box::new(effects::data_stream::DataStream::new())),
                ("tetris",        || Box::new(effects::tetris::Tetris::new())),
                ("boot-sequence", || Box::new(effects::boot_sequence::BootSequence::new())),
            ],
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.pairs.iter().map(|(k, _)| *k)
    }

    pub fn get(&self, key: &str) -> Option<EffectFactory> {
        self.pairs.iter().find(|(k, _)| *k == key).map(|(_, f)| *f)
    }

    pub fn instantiate(&self, key: &str) -> Option<Box<dyn Effect>> {
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
    fn registry_has_at_least_twelve_effects() {
        assert!(Registry::new().len() >= 12, "registry shrunk to {}", Registry::new().len());
    }

    #[test]
    fn registry_names_are_unique() {
        let r = Registry::new();
        let mut seen = std::collections::HashSet::new();
        for k in r.keys() {
            assert!(seen.insert(k), "duplicate effect name: {}", k);
        }
    }

    #[test]
    fn each_factory_produces_named_effect() {
        let r = Registry::new();
        for k in r.keys() {
            let e = r.instantiate(k).unwrap();
            assert_eq!(e.name(), k);
            assert!(!e.title().is_empty());
            assert!(!e.description().is_empty());
            assert!(e.duration().as_millis() > 0);
        }
    }
}
