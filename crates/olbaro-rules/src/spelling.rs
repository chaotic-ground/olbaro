//! 맞춤법 규칙 묶음. 어떤 문맥에서도 틀린 꼴이거나, 바로 앞 어절이 답을 정해 주는 경우만 오류로 낸다.

use olbaro_core::{Block, Diagnostic, Document, Rule, Severity, Span};

use crate::hangul::{Eojeol, RIEUL, all_syllables, eojeols, has_final, jongseong};
use crate::text_diag;

pub const GROUP: &str = "spelling";

pub fn rules() -> Vec<Box<dyn Rule>> {
    let mut rules: Vec<Box<dyn Rule>> = FIXED
        .iter()
        .map(|f| Box::new(f.clone()) as Box<dyn Rule>)
        .collect();
    rules.extend([
        Box::new(DwaeyoDwaeseo) as Box<dyn Rule>,
        Box::new(LgeEnding),
        Box::new(AnAnh),
        Box::new(Barada),
        Box::new(BaramHint),
    ]);
    rules
}

macro_rules! spelling_rule {
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

/// 문장마다 (앞 어절, 어절)을 넘긴다. 문장 첫 어절의 앞 어절은 `None`.
fn for_each_word(doc: &Document, mut f: impl FnMut(&Block, Span, Option<&Eojeol>, &Eojeol)) {
    for b in doc.blocks() {
        for s in &b.sentences {
            let words = eojeols(b.sentence_text(s));
            let base = s.text_span.start;
            for (i, w) in words.iter().enumerate() {
                let prev = i.checked_sub(1).map(|p| &words[p]);
                f(b, Span::new(base + w.start, base + w.end()), prev, w);
            }
        }
    }
}

/// 어디에 나와도 틀린 꼴과 바른 꼴. 어절 안에서 그 부분만 바꾼다.
#[derive(Clone)]
pub struct FixedMisspelling {
    id: &'static str,
    description: &'static str,
    pairs: &'static [(&'static str, &'static str)],
}

const FIXED: &[FixedMisspelling] = &[
    FixedMisspelling {
        id: "spelling.dwaess",
        description: "'됬'은 없는 글자다. '되었'의 준말은 '됐' (됬다 → 됐다)",
        pairs: &[("됬", "됐")],
    },
    FixedMisspelling {
        id: "spelling.myeochil",
        description: "'몇일'은 '며칠'로 쓴다",
        pairs: &[("몇일", "며칠")],
    },
    FixedMisspelling {
        id: "spelling.eotteokhae",
        description: "'어떻해'는 '어떡해'로 쓴다 ('어떻게 해'의 준말)",
        pairs: &[("어떻해", "어떡해"), ("어떻하", "어떡하")],
    },
    FixedMisspelling {
        id: "spelling.waen-wen",
        description: "'왠지'(왜인지)와 '웬'(어찌 된)을 바로 쓴다 (웬지 → 왠지, 왠일 → 웬일)",
        pairs: &[("웬지", "왠지"), ("왠만", "웬만"), ("왠일", "웬일")],
    },
];

impl Rule for FixedMisspelling {
    fn id(&self) -> &'static str {
        self.id
    }
    fn group(&self) -> &'static str {
        GROUP
    }
    fn description(&self) -> &'static str {
        self.description
    }

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_word(doc, |b, span, _, w| {
            let Some(&(wrong, right)) = self.pairs.iter().find(|(wrong, _)| w.text.contains(wrong))
            else {
                return;
            };
            out.push(text_diag(
                doc,
                b,
                span,
                self.id,
                Severity::Error,
                format!("'{wrong}'은 '{right}'로 씁니다"),
                Some(w.text.replace(wrong, right)),
            ));
        });
    }
}

/// "되요" → "돼요", "되서" → "돼서": '되-' 뒤에 '-어'가 줄어든 꼴은 '돼'다.
pub struct DwaeyoDwaeseo;

impl Rule for DwaeyoDwaeseo {
    spelling_rule!(
        "spelling.dwae",
        "'되어요/되어서'의 준말은 '돼요/돼서'다 (되요 → 돼요)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_word(doc, |b, span, _, w| {
            let Some(at) = ["되요", "되서", "되서는", "되서도", "되서야", "되서요"]
                .iter()
                .find_map(|end| w.text.strip_suffix(end).map(str::len))
            else {
                return;
            };
            let fixed = format!("{}돼{}", &w.text[..at], &w.text[at + '되'.len_utf8()..]);
            out.push(text_diag(
                doc,
                b,
                span,
                self.id(),
                Severity::Error,
                "'되어'가 줄면 '돼'입니다",
                Some(fixed),
            ));
        });
    }
}

/// "할께" → "할게": 어미 '-ㄹ게'는 된소리로 적지 않는다.
pub struct LgeEnding;

impl Rule for LgeEnding {
    spelling_rule!(
        "spelling.lge",
        "어미 '-ㄹ게'는 '-ㄹ께'로 적지 않는다 (할께 → 할게)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_word(doc, |b, span, _, w| {
            let Some((stem, tail)) = w
                .text
                .strip_suffix("께요")
                .map(|s| (s, "게요"))
                .or_else(|| w.text.strip_suffix('께').map(|s| (s, "게")))
            else {
                return;
            };
            // "딸께", "아들께"는 높임 조사 '께'다.
            if !all_syllables(stem)
                || !has_final(stem.chars().next_back().unwrap(), RIEUL)
                || matches!(stem, "딸" | "아들")
            {
                return;
            }
            out.push(text_diag(
                doc,
                b,
                span,
                self.id(),
                Severity::Error,
                "어미 '-ㄹ게'는 '게'로 적습니다",
                Some(format!("{stem}{tail}")),
            ));
        });
    }
}

/// '안'(아니)과 '않'(아니하)을 바꿔 쓴 것: "않되" → "안 되", "하지 안았다" → "하지 않았다".
pub struct AnAnh;

impl Rule for AnAnh {
    spelling_rule!(
        "spelling.an-anh",
        "'안'은 '아니', '않'은 '아니하'의 준말이다 (않되 → 안 되, 하지 안았다 → 하지 않았다)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_word(doc, |b, span, prev, w| {
            let fixed = if let Some(rest) = w.text.strip_prefix('않')
                && ["되", "돼", "됩", "된", "될", "됐"]
                    .iter()
                    .any(|v| rest.starts_with(v))
            {
                format!("안 {rest}")
            } else if let Some(rest) = w.text.strip_prefix('안')
                && prev.is_some_and(|p| p.text.chars().count() >= 2 && p.text.ends_with('지'))
                && [
                    "았", "아요", "아서", "고", "는", "은", "을", "습니", "게", "으면", "으니",
                ]
                .iter()
                .any(|e| rest.starts_with(e))
            {
                format!("않{rest}")
            } else {
                return;
            };
            out.push(text_diag(
                doc,
                b,
                span,
                self.id(),
                Severity::Error,
                "'안'은 '아니', '않'은 '아니하'의 준말입니다",
                Some(fixed),
            ));
        });
    }
}

/// "잘되길 바래" → "잘되길 바라": 앞 어절이 '-기를/-길'이면 '바라다'(원하다)다.
/// '바래다'(색이 바래다, 바래다주다)도 있는 말이라 단서 없이는 고치지 않는다.
pub struct Barada;

fn wish_cue(prev: &Eojeol) -> bool {
    let t = prev.text;
    if t.ends_with("기를") || t.ends_with("기만을") {
        return true;
    }
    // "되길", "있길"은 '-기를'의 준말. "골목길", "산길"처럼 받침 뒤 '길'은 대개 명사다.
    let Some(stem) = t.strip_suffix('길') else {
        return false;
    };
    stem.chars().next_back().is_some_and(|c| {
        jongseong(c) == Some(0) || matches!(c, '있' | '없' | '좋' | '받' | '않' | '싶' | '많')
    })
}

fn barada_fix(word: &str) -> Option<String> {
    [
        ("바랬", "바랐"),
        ("바램", "바람"),
        ("바랩", "바랍"),
        ("바래", "바라"),
    ]
    .iter()
    .find_map(|(wrong, right)| {
        word.strip_prefix(wrong)
            .map(|rest| format!("{right}{rest}"))
    })
}

impl Rule for Barada {
    spelling_rule!(
        "spelling.barada",
        "'-기를 바라다'는 '바래'가 아니라 '바라'로 적는다 (되길 바래 → 되길 바라)"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_word(doc, |b, span, prev, w| {
            if !prev.is_some_and(wish_cue) {
                return;
            }
            let Some(fixed) = barada_fix(w.text) else {
                return;
            };
            out.push(text_diag(
                doc,
                b,
                span,
                self.id(),
                Severity::Error,
                "'원하다'의 뜻은 '바라다'라서 '바라', '바랐다', '바람'으로 씁니다",
                Some(fixed),
            ));
        });
    }
}

/// "나의 바램은": '바라다'의 명사형은 '바람'이지만, '바래다'의 명사형일 수도 있어서 묻기만 한다.
pub struct BaramHint;

impl Rule for BaramHint {
    spelling_rule!(
        "spelling.baram-hint",
        "'바램'이 '원하는 것'을 뜻하면 '바람'인지 묻는다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for_each_word(doc, |b, span, prev, w| {
            if !w.text.starts_with("바램") || prev.is_some_and(wish_cue) {
                return;
            }
            out.push(
                text_diag(
                    doc,
                    b,
                    span,
                    self.id(),
                    Severity::Hint,
                    "혹시 '바람'?",
                    None,
                )
                .with_suggestion("'원하는 것'이라는 뜻이면 '바라다'의 명사형 '바람'으로 씁니다"),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use olbaro_core::{Config, Linter};

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

    fn assert_clean(cases: &[&str]) {
        for ok in cases {
            assert!(lint(ok).is_empty(), "{ok}: {:?}", lint(ok));
        }
    }

    #[test]
    fn fixed_misspellings() {
        assert_eq!(
            fixes("다 됬다. 몇일 걸려? 어떻해. 어떻하지? 웬지 좋다. 왠만하면 와. 왠일이야."),
            [
                "됐다",
                "며칠",
                "어떡해",
                "어떡하지",
                "왠지",
                "웬만하면",
                "웬일이야"
            ]
        );
        assert_clean(&["다 됐다. 며칠 걸려? 왠지 좋다. 웬만하면 와. 웬일이야."]);
    }

    #[test]
    fn dwae() {
        assert_eq!(
            fixes("그러면 안되요. 사용되서는 안 된다."),
            ["안돼요", "사용돼서는"]
        );
        assert_clean(&["되어요. 돼서 좋다. 되새기다. 되서다."]);
    }

    #[test]
    fn lge() {
        assert_eq!(fixes("내가 할께. 먹을께요!"), ["할게", "먹을게요"]);
        assert_clean(&["선생님께 드렸다. 딸께 주었다. 할게."]);
    }

    #[test]
    fn an_anh() {
        assert_eq!(
            fixes("그러면 않돼. 하지 안았다. 먹지 안는다."),
            ["안 돼", "않았다", "않는다"]
        );
        assert_clean(&["아기를 안았다. 그러면 안 돼. 하지 않았다."]);
    }

    #[test]
    fn barada_with_cue() {
        assert_eq!(
            fixes("잘되길 바래. 합격하기를 바랬다. 있길 바래요."),
            ["바라", "바랐다", "바라요"]
        );
        assert_clean(&["색이 바래 보인다. 골목길 바래다 주었다. 잘되길 바라."]);
    }

    #[test]
    fn baram_hint() {
        assert_eq!(
            lint("나의 바램은 하나다."),
            [("spelling.baram-hint", "바램은".into(), None)]
        );
        // 단서가 있으면 오류 규칙이 맡는다.
        assert_eq!(lint("되기를 바램.")[0].0, "spelling.barada");
    }
}
