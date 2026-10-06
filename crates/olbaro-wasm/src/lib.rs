//! 브라우저 확장이 쓰는 WASM 바인딩. 확장은 가벼워야 하므로 `Needs::Text` 규칙만 싣는다.
//!
//! 값은 JSON 문자열로 주고받는다. 오프셋은 JS 문자열과 같은 UTF-16 단위다.

use std::collections::HashMap;

use olbaro_core::{Config, Document, Linter, Needs, Rule, Severity};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct RuleInfo {
    id: &'static str,
    group: &'static str,
    description: &'static str,
    default_enabled: bool,
    enabled: bool,
}

#[derive(Serialize)]
struct Lint {
    rule: &'static str,
    group: &'static str,
    severity: &'static str,
    start: usize,
    end: usize,
    problem_text: String,
    message: String,
    suggestion: Option<String>,
    replacements: Vec<String>,
}

#[wasm_bindgen]
pub struct OlbaroLinter {
    linter: Linter,
}

#[wasm_bindgen]
impl OlbaroLinter {
    /// `config_json`은 `{"규칙 id 또는 묶음": true | false | null}`. `null`은 기본값을 따른다.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<OlbaroLinter, JsError> {
        build(config_json)
            .map(|linter| OlbaroLinter { linter })
            .map_err(|e| JsError::new(&e))
    }

    /// 실린 규칙 목록과 지금 켜졌는지.
    #[wasm_bindgen(js_name = rulesJson)]
    pub fn rules_json(&self) -> String {
        rules_json(&self.linter)
    }

    #[wasm_bindgen(js_name = lintJson)]
    pub fn lint_json(&self, text: &str) -> String {
        lint_json(&self.linter, text)
    }
}

fn text_rules() -> Vec<Box<dyn Rule>> {
    olbaro_rules::all()
        .into_iter()
        .filter(|r| r.needs() == Needs::Text)
        .collect()
}

fn build(config_json: &str) -> Result<Linter, String> {
    let settings: HashMap<String, Option<bool>> = if config_json.trim().is_empty() {
        HashMap::new()
    } else {
        serde_json::from_str(config_json).map_err(|e| e.to_string())?
    };
    let rules = text_rules();
    let mut config = Config::new();
    for (name, value) in settings {
        let Some(on) = value else { continue };
        // 규칙 id가 아니면 묶음 이름으로 본다. 모르는 이름은 아무것도 바꾸지 않는다.
        if rules.iter().any(|r| r.id() == name) {
            config.set_rule(name, on);
        } else {
            config.set_group(name, on);
        }
    }
    Ok(Linter::new(rules, config))
}

fn rules_json(linter: &Linter) -> String {
    let rules: Vec<RuleInfo> = linter
        .rules()
        .map(|(rule, enabled)| RuleInfo {
            id: rule.id(),
            group: rule.group(),
            description: rule.description(),
            default_enabled: rule.default_enabled(),
            enabled,
        })
        .collect();
    serde_json::to_string(&rules).expect("rule list serializes")
}

fn lint_json(linter: &Linter, text: &str) -> String {
    let doc = Document::parse(text);
    let groups: HashMap<&str, &str> = linter.rules().map(|(r, _)| (r.id(), r.group())).collect();
    let lints: Vec<Lint> = linter
        .lint_document(&doc)
        .into_iter()
        .map(|d| Lint {
            rule: d.rule,
            group: groups.get(d.rule).copied().unwrap_or(""),
            severity: match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Hint => "hint",
            },
            start: doc.utf16_offset(d.span.start),
            end: doc.utf16_offset(d.span.end),
            problem_text: doc.slice(d.span).to_owned(),
            message: d.message,
            suggestion: d.suggestion,
            replacements: d.replacements,
        })
        .collect();
    serde_json::to_string(&lints).expect("lints serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn lints(config: &str, text: &str) -> Vec<Value> {
        serde_json::from_str(&lint_json(&build(config).unwrap(), text)).unwrap()
    }

    #[test]
    fn groups_and_rules_toggle() {
        let text = "🙂 답은 바로 여기에 있습니다.";
        assert!(
            lints("{}", text).is_empty(),
            "llmstyle은 기본으로 꺼져 있다"
        );
        let on = lints(r#"{"llmstyle": true}"#, text);
        assert_eq!(on[0]["rule"], "llmstyle.baro");
        assert!(lints(r#"{"llmstyle": true, "llmstyle.baro": false}"#, text).is_empty());
        assert!(!lints(r#"{"llmstyle": true, "llmstyle.baro": null}"#, text).is_empty());
    }

    #[test]
    fn offsets_are_utf16() {
        let text = "🙂 답은 바로 여기에 있습니다.";
        let lint = &lints(r#"{"llmstyle.baro": true}"#, text)[0];
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let (start, end) = (
            lint["start"].as_u64().unwrap() as usize,
            lint["end"].as_u64().unwrap() as usize,
        );
        assert_eq!(
            String::from_utf16(&utf16[start..end]).unwrap(),
            lint["problem_text"]
        );
    }

    #[test]
    fn replacements_are_passed_through() {
        let lint = &lints(r#"{"llmstyle.baro": true}"#, "바로 그 사람이다.")[0];
        assert_eq!(lint["problem_text"], "바로 그");
        assert_eq!(lint["replacements"], serde_json::json!(["그"]));
    }

    #[test]
    fn rule_list_reports_state() {
        let rules: Vec<Value> =
            serde_json::from_str(&rules_json(&build(r#"{"llmstyle": true}"#).unwrap())).unwrap();
        assert!(!rules.is_empty());
        assert!(
            rules
                .iter()
                .all(|r| r["group"] == "llmstyle" && r["enabled"] == true)
        );
    }

    #[test]
    fn bad_config_is_an_error() {
        assert!(build("[").is_err());
    }
}
