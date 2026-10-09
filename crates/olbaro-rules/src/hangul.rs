//! 한글 음절과 어절을 다루는 작은 도구.

/// 한글 음절의 받침 번호(없으면 0). 한글 음절이 아니면 `None`.
pub(crate) fn jongseong(c: char) -> Option<u32> {
    let code = (c as u32).checked_sub(0xAC00)?;
    (code < 11172).then_some(code % 28)
}

pub(crate) const NIEUN: u32 = 4;
pub(crate) const RIEUL: u32 = 8;

pub(crate) fn is_syllable(c: char) -> bool {
    jongseong(c).is_some()
}

/// 받침이 `final_`인 음절인가.
pub(crate) fn has_final(c: char, final_: u32) -> bool {
    jongseong(c) == Some(final_)
}

/// 글이 한글 음절로만 되어 있는가.
pub(crate) fn all_syllables(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_syllable)
}

/// 낱말 뒤에 붙일 '은/는'.
pub(crate) fn eun_neun(word: &str) -> &'static str {
    match word.chars().next_back().and_then(jongseong) {
        Some(0) | None => "는",
        Some(_) => "은",
    }
}

/// 어절 하나. 앞뒤 문장부호와 괄호는 뗀 것이다.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Eojeol<'a> {
    pub text: &'a str,
    /// 주어진 글 안에서의 바이트 오프셋.
    pub start: usize,
}

impl Eojeol<'_> {
    pub fn end(&self) -> usize {
        self.start + self.text.len()
    }
}

/// 공백으로 나눈 어절들. 문장부호만 있는 조각은 건너뛴다.
pub(crate) fn eojeols(text: &str) -> Vec<Eojeol<'_>> {
    let mut out = Vec::new();
    let mut push = |from: usize, to: usize| {
        let piece = &text[from..to];
        let core = piece.trim_matches(|c: char| !c.is_alphanumeric());
        if !core.is_empty() {
            let lead = piece.len()
                - piece
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .len();
            out.push(Eojeol {
                text: core,
                start: from + lead,
            });
        }
    };
    let mut from = 0;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            push(from, i);
            from = i + c.len_utf8();
        }
    }
    push(from, text.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_eojeols() {
        let text = "(가는데가) 없다…  정말";
        let e = eojeols(text);
        let words: Vec<_> = e.iter().map(|w| w.text).collect();
        assert_eq!(words, ["가는데가", "없다", "정말"]);
        for w in e {
            assert_eq!(&text[w.start..w.end()], w.text);
        }
    }

    #[test]
    fn finals() {
        assert!(has_final('는', NIEUN));
        assert!(has_final('갈', RIEUL));
        assert!(!has_final('가', NIEUN));
        assert_eq!(jongseong('a'), None);
    }
}
