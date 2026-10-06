//! LLM 말투 규칙 묶음. 맞춤법이 아니라 문체라서 묶음째 기본으로 꺼 둔다.
//!
//! 전부 형태소 분석 없이 정규식, 단어 목록, 문서 구조만으로 돈다.

use std::collections::HashMap;
use std::sync::LazyLock;

use olbaro_core::{Block, BlockKind, Diagnostic, Document, Rule, Severity, Span};
use regex::Regex;

pub const GROUP: &str = "llmstyle";

pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(ShortSentenceRun::default()),
        Box::new(BoldLeadList::default()),
        Box::new(GeotIpnida),
        Box::new(SemIpnida),
        Box::new(Foreshadow),
        Box::new(BaroDeictic),
        Box::new(CountOfThings),
        Box::new(SentenceHeading),
        Box::new(RepeatedOpeningConjunction::default()),
        Box::new(GiTtaemunTwice),
    ]
}

macro_rules! style_rule {
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
        fn default_enabled(&self) -> bool {
            false
        }
    };
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("규칙 정규식")
}

/// 블록 평문에서 정규식이 맞은 곳마다 진단을 낸다. 캡처 그룹 1이 있으면 그 부분을 가리킨다.
fn report_matches(
    block: &Block,
    re: &Regex,
    rule: &'static str,
    severity: Severity,
    message: &str,
    suggestion: Option<&str>,
    out: &mut Vec<Diagnostic>,
) {
    for caps in re.captures_iter(&block.text) {
        let m = caps.get(1).unwrap_or_else(|| caps.get(0).unwrap());
        let mut d = Diagnostic::new(
            rule,
            severity,
            block.to_source(Span::new(m.start(), m.end())),
            message,
        );
        if let Some(s) = suggestion {
            d = d.with_suggestion(s);
        }
        out.push(d);
    }
}

fn prose_blocks(doc: &Document) -> impl Iterator<Item = &Block> {
    doc.blocks().iter()
}

/// 1. 짧은 평서문 늘어놓기: 한 문단에서 짧은 문장이 끊겨서 여럿 이어진다.
pub struct ShortSentenceRun {
    pub max_eojeol: usize,
    pub min_run: usize,
}

impl Default for ShortSentenceRun {
    fn default() -> Self {
        Self {
            max_eojeol: 10,
            min_run: 3,
        }
    }
}

impl Rule for ShortSentenceRun {
    style_rule!(
        "llmstyle.short-run",
        "짧은 평서문을 마침표로 끊어 늘어놓는다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in doc
            .blocks()
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Paragraph | BlockKind::BoldLead))
        {
            let mut run: Vec<Span> = Vec::new();
            let flush = |run: &mut Vec<Span>, out: &mut Vec<Diagnostic>| {
                if run.len() >= self.min_run {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Warning,
                            Span::new(run[0].start, run[run.len() - 1].end),
                            format!("짧은 평서문 {}개가 끊겨서 이어진다", run.len()),
                        )
                        .with_suggestion(
                            "붙을 것은 붙이고, 인과는 문장 안으로 넣고, 길이를 섞는다",
                        ),
                    );
                }
                run.clear();
            };
            for s in &b.sentences {
                let text = b.sentence_text(s);
                let short =
                    text.split_whitespace().count() <= self.max_eojeol && ends_declarative(text);
                if short {
                    run.push(s.span);
                } else {
                    flush(&mut run, out);
                }
            }
            flush(&mut run, out);
        }
    }
}

fn ends_declarative(sentence: &str) -> bool {
    let t = sentence
        .trim_end_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace() || c == '…');
    t.ends_with('다') || t.ends_with('요')
}

/// 2. 줄 앞에만 볼드를 넣은 나열.
pub struct BoldLeadList {
    pub min_run: usize,
}

impl Default for BoldLeadList {
    fn default() -> Self {
        Self { min_run: 2 }
    }
}

impl Rule for BoldLeadList {
    style_rule!("llmstyle.bold-lead", "줄 앞에만 볼드를 넣고 나열한다");

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        let mut run: Vec<Span> = Vec::new();
        let mut flush = |run: &mut Vec<Span>| {
            if run.len() >= self.min_run {
                for span in run.iter() {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Warning,
                            *span,
                            format!("볼드 머리 줄이 {}개 이어진다", run.len()),
                        )
                        .with_suggestion("소제목으로 올리거나 평범한 문단으로 쓴다"),
                    );
                }
            }
            run.clear();
        };
        for b in doc.blocks() {
            match b.kind {
                BlockKind::BoldLead => run.push(b.span),
                // 볼드 줄 사이에 끼는 설명 문단은 나열을 끊지 않는다.
                BlockKind::Paragraph | BlockKind::ListItem if !run.is_empty() => {}
                _ => flush(&mut run),
            }
        }
        flush(&mut run);
    }
}

static GEOT: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?:^|\s)(것(?:입니다|이었습니다|이에요|이었어요|이다|이었다|이죠|이지요))")
});
static SEM: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?:^|\s)(셈(?:입니다|이었습니다|이에요|이다|이었다|이죠|이지요))"));

/// 3. ~것입니다 꼬리. 앞에 띄어쓰기가 있어야 해서 "그것입니다"는 걸리지 않는다.
pub struct GeotIpnida;

impl Rule for GeotIpnida {
    style_rule!(
        "llmstyle.geot-ipnida",
        "평범한 서술어로 끝낼 자리를 '~것입니다'로 명사화한다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in prose_blocks(doc) {
            report_matches(
                b,
                &GEOT,
                self.id(),
                Severity::Warning,
                "'~것입니다' 꼬리",
                Some("평범한 서술어로 끝낸다(고쳐둔 것입니다 → 고쳐두었습니다)"),
                out,
            );
        }
    }
}

/// 4. ~인 셈입니다.
pub struct SemIpnida;

impl Rule for SemIpnida {
    style_rule!(
        "llmstyle.sem-ipnida",
        "앞 문장을 다시 말하며 '~인 셈입니다'로 맺는다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in prose_blocks(doc) {
            report_matches(
                b,
                &SEM,
                self.id(),
                Severity::Warning,
                "'~인 셈입니다'",
                Some("앞 문장을 되풀이하는지 보고, 그렇다면 지운다"),
                out,
            );
        }
    }
}

static FORESHADOW: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(곧 답이었|답은 간단|핵심은 (?:바로 )?여기|여기에 답이|비밀은 바로)"));

/// 5. 답을 내놓기 전에 예고하기. 의미 판단이 필요해 어휘 목록으로만 잡는다.
pub struct Foreshadow;

impl Rule for Foreshadow {
    style_rule!("llmstyle.foreshadow", "답을 내놓기 전에 예고한다");

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in prose_blocks(doc) {
            report_matches(
                b,
                &FORESHADOW,
                self.id(),
                Severity::Hint,
                "답 앞의 예고",
                Some("예고를 지우고 바로 답을 쓴다"),
                out,
            );
        }
    }
}

// 관형사 "그/이/저"는 뒤에 띄어쓰기가 와야 한다("바로 이해"는 걸리지 않게).
static BARO: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?:^|\s)(바로 (?:그|이|저)\s|바로 (?:여기|거기|저기|이것|그것|저것))")
});

/// 7. 바로 그 / 바로 여기.
pub struct BaroDeictic;

impl Rule for BaroDeictic {
    style_rule!(
        "llmstyle.baro",
        "가리킬 대상이 없는데 '바로 그/여기'로 손가락질한다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in prose_blocks(doc) {
            for caps in BARO.captures_iter(&b.text) {
                let m = caps.get(1).unwrap();
                let found = m.as_str().trim_end();
                let span = b.to_source(Span::new(m.start(), m.start() + found.len()));
                let mut d =
                    Diagnostic::new(self.id(), Severity::Warning, span, "'바로'로 가리키기")
                        .with_suggestion("'바로'를 뺀다");
                // 사이에 마크업이 끼어 있으면 바꿔 넣을 때 마크업이 지워지므로 후보를 내지 않는다.
                if doc.slice(span) == found {
                    d = d.with_replacement(found["바로".len()..].trim_start());
                }
                out.push(d);
            }
        }
    }
}

static COUNT: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?:^|\s)((?:셋|넷)(?:\s|$|[은이을입과,.])|(?:두|세|네)\s?(?:가지|개))")
});

/// 8. 개수를 세어 보이기. 소제목에 박혀 있으면 경고, 본문이면 안내.
pub struct CountOfThings;

impl Rule for CountOfThings {
    style_rule!("llmstyle.count", "'셋', '세 가지'처럼 개수를 세어 보인다");

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in prose_blocks(doc) {
            let severity = if matches!(b.kind, BlockKind::Heading(_)) {
                Severity::Warning
            } else {
                Severity::Hint
            };
            for caps in COUNT.captures_iter(&b.text) {
                let m = caps.get(1).unwrap();
                let word = m.as_str().trim_end_matches(|c: char| {
                    !matches!(c, '가'..='힣') || "은이을입과".contains(c)
                });
                let word = if word.is_empty() { m.as_str() } else { word };
                out.push(Diagnostic::new(
                    self.id(),
                    severity,
                    b.to_source(Span::new(m.start(), m.start() + word.len())),
                    "개수를 세어 보이기",
                ));
            }
        }
    }
}

/// 9. 문장형 소제목.
pub struct SentenceHeading;

impl Rule for SentenceHeading {
    style_rule!("llmstyle.sentence-heading", "소제목을 문장으로 쓴다");

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in doc
            .blocks()
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Heading(_)))
        {
            if is_sentence_like(&b.text) {
                out.push(
                    Diagnostic::new(self.id(), Severity::Warning, b.span, "문장형 소제목")
                        .with_suggestion("명사구로 바꾼다(요청을 아무 때나 보내면 안 됩니다 → 요청을 보내는 시점)"),
                );
            }
        }
    }
}

fn is_sentence_like(heading: &str) -> bool {
    let t = heading.trim_end_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());
    const ENDINGS: &[&str] = &[
        "니다", "어요", "아요", "에요", "예요", "해요", "까", "죠", "었다", "았다", "였다", "했다",
        "겠다", "는다", "있다", "없다", "이다", "않다",
    ];
    if ENDINGS.iter().any(|e| t.ends_with(e)) {
        return true;
    }
    // "한다", "된다", "간다"처럼 ㄴ 받침 음절 + 다.
    let mut chars = t.chars().rev();
    matches!((chars.next(), chars.next()), (Some('다'), Some(prev)) if jongseong(prev) == Some(4))
}

/// 한글 음절의 받침 번호(없으면 0). 4는 ㄴ.
pub(crate) fn jongseong(c: char) -> Option<u32> {
    let code = (c as u32).checked_sub(0xAC00)?;
    (code < 11172).then_some(code % 28)
}

const CONJUNCTIONS: &[&str] = &[
    "그래서",
    "그런데",
    "하지만",
    "그러나",
    "그리고",
    "따라서",
    "그러므로",
    "또한",
    "게다가",
    "결국",
    "물론",
    "즉",
];

/// 10. 문두 접속부사 반복.
pub struct RepeatedOpeningConjunction {
    pub limit: usize,
}

impl Default for RepeatedOpeningConjunction {
    fn default() -> Self {
        Self { limit: 2 }
    }
}

impl Rule for RepeatedOpeningConjunction {
    style_rule!(
        "llmstyle.opening-conj",
        "같은 접속부사로 문장을 자주 시작한다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        let mut found: Vec<(&'static str, Span)> = Vec::new();
        for b in doc.blocks() {
            for s in &b.sentences {
                let text = b.sentence_text(s);
                let first = text
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches([',', '，']);
                if let Some(&c) = CONJUNCTIONS.iter().find(|&&c| c == first) {
                    found.push((c, Span::new(s.span.start, s.span.start + c.len())));
                }
            }
        }
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (c, _) in &found {
            *counts.entry(c).or_default() += 1;
        }
        for (c, span) in found {
            let n = counts[c];
            if n > self.limit {
                out.push(
                    Diagnostic::new(
                        self.id(),
                        Severity::Hint,
                        span,
                        format!("'{c}'가 문서에서 {n}번 문장 첫머리에 온다"),
                    )
                    .with_suggestion("세어 보고 줄인다"),
                );
            }
        }
    }
}

static GI_TTAEMUN: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(기 때문(?:입니다|이다|이에요|이죠|이지요))"));

/// '~기 때문입니다'는 이유를 대는 보통 방식이라, 한 문단에 두 번 이상일 때만 짚는다.
pub struct GiTtaemunTwice;

impl Rule for GiTtaemunTwice {
    style_rule!(
        "llmstyle.gi-ttaemun",
        "한 문단에 '~기 때문입니다'가 두 번 이상 나온다"
    );

    fn check(&self, doc: &Document, out: &mut Vec<Diagnostic>) {
        for b in prose_blocks(doc) {
            if GI_TTAEMUN.find_iter(&b.text).count() >= 2 {
                report_matches(
                    b,
                    &GI_TTAEMUN,
                    self.id(),
                    Severity::Hint,
                    "한 문단에 '~기 때문입니다'가 두 번 이상",
                    None,
                    out,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use olbaro_core::{Config, Linter};

    fn lint(src: &str) -> Vec<(&'static str, String)> {
        let mut config = Config::new();
        config.set_group(GROUP, true);
        let doc = Document::parse(src);
        Linter::new(rules(), config)
            .lint_document(&doc)
            .into_iter()
            .map(|d| (d.rule, doc.slice(d.span).to_owned()))
            .collect()
    }

    fn ids(src: &str) -> Vec<&'static str> {
        lint(src).into_iter().map(|(id, _)| id).collect()
    }

    #[test]
    fn off_by_default() {
        let doc = Document::parse("고쳐둔 것입니다.");
        assert!(
            Linter::new(rules(), Config::new())
                .lint_document(&doc)
                .is_empty()
        );
    }

    #[test]
    fn guide_example_before_is_short_run_after_is_not() {
        let before = "드보락에서 모든 단축키가 QWERTY 자리에 있었습니다. Ctrl+V가 v를 치는 키가 아니라 각인이 V인 키에서 동작했습니다. 스캔코드는 제대로 도착하고 드라이버의 드보락 표도 맞습니다. 그 표가 안 쓰였습니다.";
        let after = "드보락을 쓰는데도 단축키가 전부 QWERTY 자리에 있었습니다. Ctrl+V는 v를 치는 키가 아니라 각인이 V인 키에서 동작했고, 나머지도 사정이 같았습니다. 스캔코드는 제대로 도착하고 드라이버 안의 드보락 표도 멀쩡한데, 정작 그 표가 쓰이지 않고 있었던 것입니다.";
        assert_eq!(ids(before), vec!["llmstyle.short-run"]);
        assert!(!ids(after).contains(&"llmstyle.short-run"));
    }

    #[test]
    fn bold_lead_needs_two() {
        assert!(ids("**하나.** 설명입니다.").is_empty());
        let two = "**이모지는 흑백입니다.** 색만 빠집니다.\n\n**입력기는 멀쩡합니다.** 문제는 다른 데 있었습니다.";
        assert_eq!(ids(two), vec!["llmstyle.bold-lead", "llmstyle.bold-lead"]);
    }

    #[test]
    fn geot_and_sem() {
        assert_eq!(
            lint("고쳐둔 것입니다."),
            vec![("llmstyle.geot-ipnida", "것입니다".into())]
        );
        assert!(
            ids("바로 그것입니다.")
                .iter()
                .all(|&i| i != "llmstyle.geot-ipnida")
        );
        assert_eq!(
            lint("통로가 없는 셈입니다."),
            vec![("llmstyle.sem-ipnida", "셈입니다".into())]
        );
    }

    #[test]
    fn baro_offers_replacement_only_on_plain_text() {
        let fix = |src: &str| -> Vec<Vec<String>> {
            let mut config = Config::new();
            config.set_rule("llmstyle.baro", true);
            Linter::new(rules(), config)
                .lint(src)
                .into_iter()
                .map(|d| d.replacements)
                .collect()
        };
        assert_eq!(
            fix("바로 그 지점을 찾았습니다."),
            vec![vec!["그".to_owned()]]
        );
        assert_eq!(
            fix("답은 바로 여기에 있습니다."),
            vec![vec!["여기".to_owned()]]
        );
        assert_eq!(fix("바로 **그** 지점입니다."), vec![Vec::<String>::new()]);
    }

    #[test]
    fn baro() {
        assert_eq!(
            lint("바로 그 지점을 찾았습니다."),
            vec![("llmstyle.baro", "바로 그".into())]
        );
        assert!(ids("바로 이해했습니다.").is_empty());
        assert_eq!(ids("답은 바로 여기에 있습니다."), vec!["llmstyle.baro"]);
    }

    #[test]
    fn count_in_heading_is_warning() {
        let doc = Document::parse("## 알아둘 결정 셋\n\n이유는 세 가지입니다.\n\n리셋했습니다.");
        let mut config = Config::new();
        config.set_group(GROUP, true);
        let got: Vec<(Severity, String)> = Linter::new(rules(), config)
            .lint_document(&doc)
            .into_iter()
            .filter(|d| d.rule == "llmstyle.count")
            .map(|d| (d.severity, doc.slice(d.span).to_owned()))
            .collect();
        assert_eq!(
            got,
            vec![
                (Severity::Warning, "셋".into()),
                (Severity::Hint, "세 가지".into())
            ]
        );
    }

    #[test]
    fn sentence_heading() {
        assert_eq!(
            ids("## 요청을 아무 때나 보내면 안 됩니다"),
            vec!["llmstyle.sentence-heading"]
        );
        assert_eq!(
            ids("### 낡은 토큰이 트레이 메뉴를 죽였다"),
            vec!["llmstyle.sentence-heading"]
        );
        assert_eq!(
            ids("== 문제가 생긴다 =="),
            vec!["llmstyle.sentence-heading"]
        );
        assert!(ids("## 요청을 보내는 시점").is_empty());
        assert!(ids("## 바다").is_empty());
    }

    #[test]
    fn opening_conjunction_counts_whole_document() {
        let src = "그래서 고쳤습니다. 길게 이어지는 문장을 하나 넣어서 짧은 문장 규칙이 걸리지 않게 합니다 정말로 길게요.\n\n그래서 다시 봤습니다, 이번에는 아주 오래 들여다보면서 하나하나 확인하고 기록했습니다.\n\n그래서, 끝났습니다 그리고 이 문장도 충분히 길어서 다른 규칙에는 걸리지 않을 겁니다 아마도요.";
        assert_eq!(ids(src), vec!["llmstyle.opening-conj"; 3]);
        assert!(ids("그래서 고쳤습니다. 그래서 다시 봤는데 이 문장은 충분히 길어서 다른 규칙에 걸리지 않습니다.").is_empty());
    }

    #[test]
    fn gi_ttaemun_only_when_twice_in_paragraph() {
        assert!(ids("잠금이 먼저 풀리기 때문입니다.").is_empty());
        assert_eq!(
            ids(
                "순서가 중요한데 잠금이 먼저 풀리기 때문입니다. 그 뒤로 창이 늦게 뜨는 것도 화면이 그 사이에 다시 그려지기 때문입니다."
            ),
            vec!["llmstyle.gi-ttaemun"; 2]
        );
    }

    #[test]
    fn code_is_skipped() {
        assert!(ids("```\n고쳐둔 것입니다.\n```\n\n인라인 `고쳐둔 것입니다` 코드는 건너뛰고 나머지 문장만 검사하게 됩니다.").is_empty());
    }
}
