//! 올바로 규칙 묶음.

pub mod llmstyle;

use olbaro_core::Rule;

/// 모든 규칙. 켜고 끄기는 `olbaro_core::Config`가 정한다.
pub fn all() -> Vec<Box<dyn Rule>> {
    llmstyle::rules()
}
