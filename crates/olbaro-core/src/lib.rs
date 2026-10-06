//! 올바로 코어. 문서를 한 번 나눠 두고(마크다운 구조, 문단, 문장), 규칙들이 그 결과를 읽어 진단을 낸다.
//!
//! 코어는 WASM에서도 돌아야 하므로 파일·네트워크·스레드를 쓰지 않는다.

mod config;
mod diagnostic;
mod document;
mod linter;
mod rule;

pub use config::Config;
pub use diagnostic::{Diagnostic, Severity};
pub use document::{Block, BlockKind, Document, Sentence, Span};
pub use linter::Linter;
pub use rule::{Needs, Rule};
