use crate::{Diagnostic, Document};

/// 규칙이 돌기 위해 필요한 것. 확장·PWA처럼 가벼워야 하는 곳은 `Text` 규칙만 켤 수 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Needs {
    /// 정규식, 단어 목록, 문서 구조만.
    Text,
    /// 따로 받는 철자 사전.
    Dict,
    /// 형태소 분석기(Kiwi). 네이티브 빌드에서만.
    Kiwi,
}

pub trait Rule: Send + Sync {
    /// 바뀌지 않는 id. 설정 파일과 문서 안 주석이 이 id로 규칙을 가리킨다.
    fn id(&self) -> &'static str;
    /// 묶음 이름. 묶음째 켜고 끌 수 있다.
    fn group(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_enabled(&self) -> bool {
        true
    }
    fn needs(&self) -> Needs {
        Needs::Text
    }
    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>);
}
