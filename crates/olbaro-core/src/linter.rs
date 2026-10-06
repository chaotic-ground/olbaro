use crate::{Config, Diagnostic, Document, Rule};

pub struct Linter {
    rules: Vec<Box<dyn Rule>>,
    config: Config,
}

impl Linter {
    pub fn new(rules: Vec<Box<dyn Rule>>, config: Config) -> Self {
        Self { rules, config }
    }

    pub fn rules(&self) -> impl Iterator<Item = (&dyn Rule, bool)> {
        self.rules
            .iter()
            .map(|r| (r.as_ref(), self.config.is_enabled(r.as_ref())))
    }

    pub fn lint_document(&self, doc: &Document) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for rule in &self.rules {
            if self.config.is_enabled(rule.as_ref()) {
                rule.check(doc, &mut out);
            }
        }
        out.sort_by_key(|d| (d.span.start, d.span.end, d.rule));
        out
    }

    pub fn lint(&self, source: &str) -> Vec<Diagnostic> {
        self.lint_document(&Document::parse(source))
    }
}
