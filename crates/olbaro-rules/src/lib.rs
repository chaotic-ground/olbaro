//! 올바로 규칙 묶음.

mod hangul;
pub mod llmstyle;
pub mod spacing;
pub mod spelling;

use olbaro_core::{Block, Diagnostic, Document, Rule, Severity, Span};

/// 모든 규칙. 켜고 끄기는 `olbaro_core::Config`가 정한다.
pub fn all() -> Vec<Box<dyn Rule>> {
    let mut rules = spelling::rules();
    rules.extend(spacing::rules());
    rules.extend(llmstyle::rules());
    rules
}

/// 블록 평문의 `text_span`을 가리키는 진단. `replacement`는 그 자리에 넣을 글이다.
pub(crate) fn text_diag(
    doc: &Document,
    block: &Block,
    text_span: Span,
    rule: &'static str,
    severity: Severity,
    message: impl Into<String>,
    replacement: Option<String>,
) -> Diagnostic {
    let span = block.to_source(text_span);
    let mut d = Diagnostic::new(rule, severity, span, message);
    // 사이에 마크업이 끼어 있으면 바꿔 넣을 때 마크업이 지워지므로 후보를 내지 않는다.
    if let Some(r) = replacement
        && doc.slice(span) == &block.text[text_span.start..text_span.end]
    {
        d = d.with_replacement(r);
    }
    d
}
