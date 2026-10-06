use std::collections::HashMap;

use crate::Rule;

/// 규칙별·묶음별 켜고 끄기. 규칙 id 설정이 묶음 설정보다, 묶음 설정이 규칙 기본값보다 앞선다.
#[derive(Debug, Clone, Default)]
pub struct Config {
    rules: HashMap<String, bool>,
    groups: HashMap<String, bool>,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_rule(&mut self, id: impl Into<String>, enabled: bool) -> &mut Self {
        self.rules.insert(id.into(), enabled);
        self
    }

    pub fn set_group(&mut self, group: impl Into<String>, enabled: bool) -> &mut Self {
        self.groups.insert(group.into(), enabled);
        self
    }

    pub fn is_enabled(&self, rule: &dyn Rule) -> bool {
        self.rules
            .get(rule.id())
            .or_else(|| self.groups.get(rule.group()))
            .copied()
            .unwrap_or_else(|| rule.default_enabled())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagnostic, Document};

    struct Dummy;
    impl Rule for Dummy {
        fn id(&self) -> &'static str {
            "g.dummy"
        }
        fn group(&self) -> &'static str {
            "g"
        }
        fn description(&self) -> &'static str {
            ""
        }
        fn default_enabled(&self) -> bool {
            false
        }
        fn check(&self, _: &Document, _: &mut Vec<Diagnostic>) {}
    }

    #[test]
    fn rule_overrides_group_overrides_default() {
        let mut c = Config::new();
        assert!(!c.is_enabled(&Dummy));
        c.set_group("g", true);
        assert!(c.is_enabled(&Dummy));
        c.set_rule("g.dummy", false);
        assert!(!c.is_enabled(&Dummy));
    }
}
