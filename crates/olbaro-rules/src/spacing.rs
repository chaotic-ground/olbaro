//! 띄어쓰기 규칙 묶음. 의존 명사(데, 만큼, 뿐, 대로, 것, 수)를 앞말에 붙여 쓴 것을 잡는다.
//!
//! 앞말이 꾸미는 말(관형형)이라는 것이 겉으로 확실할 때만 오류로 낸다.
//! "가는데"처럼 어미와 의존 명사가 모양이 같아 겉으로 갈리지 않으면,
//! 뒤따르는 말이 단서가 될 때만 안내로 묻는다.

use olbaro_core::{Diagnostic, Document, Rule, Severity, Span};

use crate::hangul::{Eojeol, NIEUN, RIEUL, all_syllables, eojeols, eun_neun, has_final};
use crate::text_diag;

pub const GROUP: &str = "spacing";

pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(DeWithParticle),
        Box::new(DeHint),
        Box::new(BoundNoun),
        Box::new(SuItda),
    ]
}

macro_rules! spacing_rule {
    ($id:literal, $desc:literal) => {
        fn id(&self) -> &'static str {
            $id
        }
        fn group(&self) -> &'static str {
            GROUP
        }
        fn description(&self) -> &'static str {
            $desc
        }
    };
}

/// 문장마다 어절 목록과, 블록 평문 안에서 그 문장이 시작하는 오프셋을 넘긴다.
fn for_each_sentence(doc: &Document, mut f: impl FnMut(&olbaro_core::Block, usize, &[Eojeol])) {
    for b in doc.blocks() {
        for s in &b.sentences {
            let words = eojeols(b.sentence_text(s));
            f(b, s.text_span.start, &words);
        }
    }
}

fn word_span(base: usize, w: &Eojeol) -> Span {
    Span::new(base + w.start, base + w.end())
}

fn last_char(s: &str) -> Option<char> {
    s.chars().next_back()
}

/// '데' 앞말이 꾸미는 말처럼 끝나는가: -는, -던, ㄴ·ㄹ 받침.
/// "몇 군데", "한데(바깥)", "본데(보고 배운 예절)"처럼 '데'로 끝나는 명사는 뺀다.
fn de_stem(stem: &str) -> bool {
    if !all_syllables(stem) || matches!(stem, "한" | "본") || stem.ends_with('군') {
        return false;
    }
    let c = last_char(stem).unwrap();
    matches!(c, '는' | '던') || has_final(c, NIEUN) || has_final(c, RIEUL)
}

/// 어절이 `stem + 데 + 조사`로 끝나면 (stem, 조사).
fn split_de(word: &str) -> Option<(&str, &str)> {
    let (stem, particle) = word.rsplit_once('데')?;
    // "-는데도", "-는데야"는 어미라서 넣지 않는다. "아는데로"는 '아는 대로'를 잘못 쓴 것일 때가 많아 뺀다.
    const PARTICLES: &[&str] = &[
        "가", "를", "는", "에", "에서", "에는", "에도", "에만", "만", "까지", "밖에", "조차",
        "부터",
    ];
    (PARTICLES.contains(&particle) && de_stem(stem)).then_some((stem, particle))
}

/// "가는데가 없다" → "가는 데가": '데' 바로 뒤에 조사가 붙으면 '데'는 의존 명사다.
pub struct DeWithParticle;

impl Rule for DeWithParticle {
    spacing_rule!(
        "spacing.de",
        "'데' 뒤에 조사가 붙으면 의존 명사이므로 띄어 쓴다 (가는데가 → 가는 데가)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_sentence(doc, |b, base, words| {
            for w in words {
                let Some((stem, particle)) = split_de(w.text) else {
                    continue;
                };
                out.push(text_diag(
                    doc,
                    b,
                    word_span(base, w),
                    self.id(),
                    Severity::Error,
                    "뒤에 조사가 붙은 '데'는 의존 명사라서 띄어 씁니다",
                    Some(format!("{stem} 데{particle}")),
                ));
            }
        });
    }
}

/// "가는데 30분 걸린다": 어미 '-는데'일 수도 있어서 고치지 않고 묻는다.
/// 뒤에 '걸리다/필요하다/쓰이다/도움/들다'가 오면 '일·곳·경우'를 뜻했을 가능성이 높다.
pub struct DeHint;

const DE_CUES: &[&str] = &[
    "걸리",
    "걸려",
    "걸렸",
    "걸린",
    "걸릴",
    "필요",
    "쓰이",
    "쓰여",
    "쓰였",
    "쓰인",
    "쓰일",
    "도움",
    "든다",
    "들었",
    "듭니다",
    "드는",
    "들어요",
    "들고",
];

impl Rule for DeHint {
    spacing_rule!(
        "spacing.de-hint",
        "'가는데 30분 걸린다'처럼 '데'가 '일·곳·경우'일 가능성이 높으면 띄어 써야 하는지 묻는다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_sentence(doc, |b, base, words| {
            for (i, w) in words.iter().enumerate() {
                let Some(stem) = w.text.strip_suffix('데') else {
                    continue;
                };
                if !de_stem(stem) {
                    continue;
                }
                let cued = words[i + 1..]
                    .iter()
                    .take(3)
                    .any(|next| DE_CUES.iter().any(|c| next.text.starts_with(c)));
                if !cued {
                    continue;
                }
                out.push(
                    text_diag(
                        doc,
                        b,
                        word_span(base, w),
                        self.id(),
                        Severity::Hint,
                        format!("혹시 '{stem} 데'?"),
                        None,
                    )
                    .with_suggestion(format!(
                        "'{stem} 일에(곳에, 경우에)'로 바꿔 말이 되면 '{stem} 데'로 띄어 씁니다"
                    )),
                );
            }
        });
    }
}

/// 꾸미는 말 뒤에 붙여 쓴 의존 명사: "먹을만큼" → "먹을 만큼", "하는것" → "하는 것".
///
/// 앞말이 '-는', '-던', '-을', '할/될'로 끝날 때만 본다. "하늘만큼", "법대로"처럼
/// 명사 뒤의 만큼·대로는 조사라서 붙여 쓰는 것이 맞으므로, 받침 ㄴ·ㄹ만으로는 판단하지 않는다.
/// 그것까지 가리려면 사전이 필요하다.
pub struct BoundNoun;

const BOUND_NOUNS: &[&str] = &["만큼", "뿐", "대로", "것", "거"];

/// '을'로 끝나는 명사. "마을만큼"의 만큼은 조사다.
const EUL_NOUNS: &[&str] = &["마을", "가을", "고을", "노을"];

/// '할'로 끝나는 명사. "역할대로"의 대로는 조사다.
const HAL_NOUNS: &[&str] = &["역할", "분할"];

fn modifier_stem(stem: &str) -> bool {
    if !all_syllables(stem) {
        return false;
    }
    match last_char(stem).unwrap() {
        '는' | '던' => true,
        '을' => !EUL_NOUNS.iter().any(|n| stem.ends_with(n)),
        '할' => !HAL_NOUNS.iter().any(|n| stem.ends_with(n)),
        '될' => true,
        _ => false,
    }
}

fn split_bound_noun(word: &str) -> Option<(&str, &'static str, &str)> {
    for &noun in BOUND_NOUNS {
        for (at, _) in word.match_indices(noun) {
            let (stem, rest) = (&word[..at], &word[at + noun.len()..]);
            if stem.is_empty() || !modifier_stem(stem) {
                continue;
            }
            if !rest.is_empty() && !all_syllables(rest) {
                continue;
            }
            // "-ㄹ뿐더러"는 어미, "먹을거리"는 한 낱말이다.
            if (noun == "뿐" && rest.starts_with("더러"))
                || (noun == "거" && rest.starts_with('리'))
            {
                continue;
            }
            return Some((stem, noun, rest));
        }
    }
    None
}

impl Rule for BoundNoun {
    spacing_rule!(
        "spacing.bound-noun",
        "꾸미는 말 뒤의 의존 명사(만큼, 뿐, 대로, 것)는 띄어 쓴다 (먹을만큼 → 먹을 만큼)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_sentence(doc, |b, base, words| {
            for w in words {
                let Some((stem, noun, rest)) = split_bound_noun(w.text) else {
                    continue;
                };
                out.push(text_diag(
                    doc,
                    b,
                    word_span(base, w),
                    self.id(),
                    Severity::Error,
                    format!("꾸미는 말 뒤의 '{noun}'{} 띄어 씁니다", eun_neun(noun)),
                    Some(format!("{stem} {noun}{rest}")),
                ));
            }
        });
    }
}

/// "할수 있다" → "할 수 있다": ㄹ 받침 뒤 '수'에 '있다/없다'가 이어지면 '수'는 의존 명사다.
pub struct SuItda;

/// ㄹ 받침 + 수로 된 명사. "홀수", "일수(日數)", "말수", "술수", "날수".
const SU_NOUN_STEMS: &[&str] = &["홀", "일", "말", "술", "날"];

impl Rule for SuItda {
    spacing_rule!(
        "spacing.su",
        "'-ㄹ 수 있다/없다'의 '수'는 띄어 쓴다 (할수 있다 → 할 수 있다)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_sentence(doc, |b, base, words| {
            for pair in words.windows(2) {
                let (w, next) = (&pair[0], &pair[1]);
                let Some(stem) = w.text.strip_suffix('수') else {
                    continue;
                };
                if !all_syllables(stem)
                    || !has_final(last_char(stem).unwrap(), RIEUL)
                    || SU_NOUN_STEMS.contains(&stem)
                    || !(next.text.starts_with('있') || next.text.starts_with('없'))
                {
                    continue;
                }
                out.push(text_diag(
                    doc,
                    b,
                    word_span(base, w),
                    self.id(),
                    Severity::Error,
                    "'-ㄹ 수 있다/없다'의 '수'는 의존 명사라서 띄어 씁니다",
                    Some(format!("{stem} 수")),
                ));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use olbaro_core::{Config, Linter};

    /// (규칙, 원문 조각, 첫 번째 바꿀 글)
    fn lint(src: &str) -> Vec<(&'static str, String, Option<String>)> {
        let doc = Document::parse(src);
        Linter::new(rules(), Config::new())
            .lint_document(&doc)
            .into_iter()
            .map(|d| {
                (
                    d.rule,
                    doc.slice(d.span).to_owned(),
                    d.replacements.first().cloned(),
                )
            })
            .collect()
    }

    fn fixes(src: &str) -> Vec<String> {
        lint(src).into_iter().filter_map(|(_, _, r)| r).collect()
    }

    #[test]
    fn de_with_particle() {
        assert_eq!(
            fixes("갈데가 없다. 먹는데에 정신이 팔렸다. 아픈데를 찾았다."),
            ["갈 데가", "먹는 데에", "아픈 데를"]
        );
        assert_eq!(
            lint("(그런데가) 있다.")[0],
            ("spacing.de", "그런데가".into(), Some("그런 데가".into()))
        );
    }

    #[test]
    fn de_endings_and_nouns_pass() {
        for ok in [
            "비가 오는데도 갔다.",
            "그렇게 말하는데야 어쩌겠어.",
            "몇 군데가 남았다.",
            "한데에서 잤다.",
            "본데가 없다.",
            "학교에 가는데 비가 왔다.",
            "가는 데가 없다.",
            "안데스가 높다.",
        ] {
            assert!(lint(ok).is_empty(), "{ok}: {:?}", lint(ok));
        }
    }

    #[test]
    fn de_hint_with_cue_verb() {
        assert_eq!(
            lint("학교 가는데 30분 걸린다."),
            [("spacing.de-hint", "가는데".into(), None)]
        );
        assert_eq!(
            lint("이걸 만드는데 돈이 많이 든다.")[0].0,
            "spacing.de-hint"
        );
        // 단서가 다른 문장에 있으면 묻지 않는다.
        assert!(lint("학교에 가는데. 30분 걸린다.").is_empty());
    }

    #[test]
    fn bound_nouns_after_modifier() {
        assert_eq!(
            fixes("먹을만큼 먹었다. 웃을뿐이다. 하던대로 해. 하는것이 낫다. 할거야. 될대로 돼라."),
            [
                "먹을 만큼",
                "웃을 뿐이다",
                "하던 대로",
                "하는 것이",
                "할 거야",
                "될 대로"
            ]
        );
    }

    #[test]
    fn particles_after_nouns_pass() {
        for ok in [
            "하늘만큼 땅만큼 좋아.",
            "법대로 하자.",
            "마을만큼 컸다.",
            "역할대로 나눴다.",
            "그것뿐이다.",
            "먹을거리가 많다.",
            "했을뿐더러 늦었다.",
            "먹을 만큼 먹었다.",
        ] {
            assert!(lint(ok).is_empty(), "{ok}: {:?}", lint(ok));
        }
    }

    #[test]
    fn su_before_itda() {
        assert_eq!(fixes("할수 있다. 먹을수 없었다."), ["할 수", "먹을 수"]);
        for ok in ["홀수 없다.", "할수록 좋다.", "갈수 밖에.", "할 수 있다."] {
            assert!(lint(ok).is_empty(), "{ok}: {:?}", lint(ok));
        }
    }

    #[test]
    fn no_replacement_across_markup() {
        let found = lint("**갈**데가 없다.");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, None);
    }
}
