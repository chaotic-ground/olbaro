use crate::Span;

/// 진단의 단계. 확실한 겉 단서가 있을 때만 `Error`, 문체는 `Warning`,
/// 둘 다 맞는 말이지만 한쪽일 가능성이 높아 묻기만 할 때는 `Hint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub severity: Severity,
    /// 원문 기준 바이트 범위.
    pub span: Span,
    pub message: String,
    /// 사람이 읽는 고치는 방법("'바로'를 뺀다"). 원문에 그대로 넣을 글이 아니다.
    pub suggestion: Option<String>,
    /// `span`을 그대로 바꿔 넣을 후보들. 비어 있으면 한 번에 고칠 수 없는 진단이다.
    pub replacements: Vec<String>,
}

impl Diagnostic {
    pub fn new(
        rule: &'static str,
        severity: Severity,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            message: message.into(),
            suggestion: None,
            replacements: Vec::new(),
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacements.push(replacement.into());
        self
    }
}
