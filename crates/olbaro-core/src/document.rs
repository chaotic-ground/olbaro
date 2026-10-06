use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// 원문 기준 바이트 범위 `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// 마크다운 `#` 소제목, 또는 위키식 `== 소제목 ==`.
    Heading(u8),
    Paragraph,
    ListItem,
    /// `**요점.** 설명` 처럼 볼드로 시작하고 뒤에 글이 이어지는 문단이나 항목.
    BoldLead,
}

/// 원문의 한 블록. `span`은 마크업까지 포함한 원문 범위다. `text`는 마크업과 인라인 코드를 뺀 평문이고, 오프셋은 `to_source`로 원문에 되돌린다.
#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    pub span: Span,
    pub text: String,
    pub sentences: Vec<Sentence>,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub struct Sentence {
    /// 블록 평문 안에서의 범위.
    pub text_span: Span,
    /// 원문 기준 범위.
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    text_start: usize,
    src_start: usize,
    len: usize,
}

impl Block {
    /// 블록 평문 안의 범위를 원문 범위로 바꾼다.
    pub fn to_source(&self, text_span: Span) -> Span {
        let start = self.map(text_span.start, false);
        let end = if text_span.end > text_span.start {
            self.map(text_span.end, true)
        } else {
            start
        };
        Span::new(start, end)
    }

    pub fn sentence_text(&self, s: &Sentence) -> &str {
        &self.text[s.text_span.start..s.text_span.end]
    }

    fn map(&self, pos: usize, is_end: bool) -> usize {
        // 끝 위치는 앞 조각에 붙이고, 시작 위치는 뒤 조각에 붙인다.
        let seg = self
            .segments
            .iter()
            .rev()
            .find(|s| {
                if is_end {
                    s.text_start < pos
                } else {
                    s.text_start <= pos
                }
            })
            .or(self.segments.first());
        match seg {
            Some(s) => s.src_start + (pos - s.text_start).min(s.len),
            None => self.span.start,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    source: String,
    blocks: Vec<Block>,
}

impl Document {
    pub fn parse(source: &str) -> Self {
        let mut builder = Builder::default();
        let parser = Parser::new_ext(
            source,
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
        );
        for (event, range) in parser.into_offset_iter() {
            builder.event(event, range.start, range.end);
        }
        let mut blocks = builder.blocks;
        for b in &mut blocks {
            promote_wiki_heading(b);
            b.sentences = split_sentences(b);
        }
        Self {
            source: source.to_owned(),
            blocks,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn slice(&self, span: Span) -> &str {
        &self.source[span.start..span.end]
    }

    /// 바이트 오프셋을 UTF-16 오프셋으로 바꾼다. 브라우저(JS 문자열)로 넘길 때 쓴다.
    pub fn utf16_offset(&self, byte: usize) -> usize {
        self.source[..byte].encode_utf16().count()
    }
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    current: Option<Block>,
    in_code_block: bool,
    strong_depth: usize,
    lead: Lead,
}

/// 지금 블록이 볼드로 시작했는지, 그 볼드가 닫혔는지.
#[derive(Default, PartialEq)]
enum Lead {
    #[default]
    Unknown,
    NotBold,
    InBold,
    BoldClosed,
}

impl Builder {
    fn open(&mut self, kind: BlockKind, start: usize) {
        self.close();
        self.current = Some(Block {
            kind,
            span: Span::new(start, start),
            text: String::new(),
            sentences: Vec::new(),
            segments: Vec::new(),
        });
        self.lead = Lead::Unknown;
    }

    fn close(&mut self) {
        if let Some(mut b) = self.current.take() {
            if b.text.trim().is_empty() {
                return;
            }
            if let Some(last) = b.segments.last() {
                b.span.end = b.span.end.max(last.src_start + last.len);
            }
            self.blocks.push(b);
        }
    }

    fn push_text(&mut self, text: &str, src_start: usize, src_len: usize) {
        let Some(b) = self.current.as_mut() else {
            return;
        };
        match self.lead {
            Lead::Unknown => {
                self.lead = if self.strong_depth > 0 {
                    Lead::InBold
                } else {
                    Lead::NotBold
                };
            }
            Lead::BoldClosed
                if !text.trim().is_empty()
                    && matches!(b.kind, BlockKind::Paragraph | BlockKind::ListItem) =>
            {
                b.kind = BlockKind::BoldLead;
            }
            _ => {}
        }
        b.segments.push(Segment {
            text_start: b.text.len(),
            src_start,
            len: src_len.min(text.len()),
        });
        b.text.push_str(text);
        b.span.end = src_start + src_len;
    }

    fn event(&mut self, event: Event, start: usize, end: usize) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                self.close();
                self.in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => self.in_code_block = false,
            Event::Start(Tag::Heading { level, .. }) => {
                self.open(BlockKind::Heading(heading_level(level)), start)
            }
            Event::Start(Tag::Paragraph) => {
                // 느슨한 목록의 항목 안 문단은 항목 블록을 그대로 쓴다.
                let in_empty_item = matches!(&self.current, Some(b) if b.kind == BlockKind::ListItem && b.text.is_empty());
                if !in_empty_item {
                    self.open(BlockKind::Paragraph, start);
                }
            }
            Event::Start(Tag::Item) => self.open(BlockKind::ListItem, start),
            Event::Start(Tag::TableCell) => self.open(BlockKind::Paragraph, start),
            Event::End(
                TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::Item | TagEnd::TableCell,
            ) => self.close(),
            Event::Start(Tag::Strong) => self.strong_depth += 1,
            Event::End(TagEnd::Strong) => {
                self.strong_depth = self.strong_depth.saturating_sub(1);
                if self.strong_depth == 0 && self.lead == Lead::InBold {
                    self.lead = Lead::BoldClosed;
                }
            }
            Event::Text(t) if !self.in_code_block => {
                // 원문과 길이가 다르면(엔티티 등) 원문 길이를 넘지 않게 맞춘다.
                self.push_text(&t, start, end - start);
            }
            Event::SoftBreak | Event::HardBreak => {
                if !self.in_code_block {
                    self.push_text(" ", start, (end - start).min(1));
                }
            }
            // 인라인 코드는 검사하지 않지만, 앞뒤 낱말이 붙지 않게 자리만 남긴다.
            Event::Code(_) => self.push_text(" ", start, 0),
            _ => {}
        }
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// `== 소제목 ==` 한 줄짜리 문단을 소제목으로 바꾼다.
fn promote_wiki_heading(b: &mut Block) {
    if b.kind != BlockKind::Paragraph {
        return;
    }
    let t = b.text.trim();
    let lead = t.chars().take_while(|&c| c == '=').count();
    let trail = t.chars().rev().take_while(|&c| c == '=').count();
    if lead >= 2 && lead == trail && t.len() > lead * 2 && !t.contains('\n') {
        let inner_start = b
            .text
            .find(|c: char| c != '=' && !c.is_whitespace())
            .unwrap_or(0);
        let inner_end = b
            .text
            .trim_end_matches(|c: char| c == '=' || c.is_whitespace())
            .len();
        if inner_start < inner_end {
            let src = b.to_source(Span::new(inner_start, inner_end));
            b.kind = BlockKind::Heading(lead as u8);
            b.text = b.text[inner_start..inner_end].to_owned();
            b.segments = vec![Segment {
                text_start: 0,
                src_start: src.start,
                len: src.end - src.start,
            }];
        }
    }
}

const TERMINATORS: &[char] = &['.', '?', '!', '…', '。'];
const CLOSERS: &[char] = &['"', '\'', '”', '’', ')', '」', '』'];

fn split_sentences(b: &Block) -> Vec<Sentence> {
    let text = &b.text;
    let mut out = Vec::new();
    let mut start = None;
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if start.is_none() {
            if c.is_whitespace() {
                continue;
            }
            start = Some(i);
        }
        if !TERMINATORS.contains(&c) {
            continue;
        }
        let mut end = i + c.len_utf8();
        while let Some(&(j, n)) = iter.peek() {
            if TERMINATORS.contains(&n) || CLOSERS.contains(&n) {
                end = j + n.len_utf8();
                iter.next();
            } else {
                break;
            }
        }
        // 마침표 뒤가 공백이나 끝일 때만 문장을 끊는다("3.5", "v1.2"는 그대로).
        if iter.peek().is_none_or(|&(_, n)| n.is_whitespace()) {
            push_sentence(b, &mut out, start.take().unwrap(), end);
        }
    }
    if let Some(s) = start {
        let end = text.trim_end().len();
        if end > s {
            push_sentence(b, &mut out, s, end);
        }
    }
    out
}

fn push_sentence(b: &Block, out: &mut Vec<Sentence>, start: usize, end: usize) {
    let text_span = Span::new(start, end);
    out.push(Sentence {
        text_span,
        span: b.to_source(text_span),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(doc: &Document) -> Vec<BlockKind> {
        doc.blocks().iter().map(|b| b.kind).collect()
    }

    #[test]
    fn blocks_and_kinds() {
        let src = "## 소제목\n\n**요점.** 설명입니다.\n\n- 항목 하나\n- **볼드만**\n\n```\n코드입니다.\n```\n\n== 위키 소제목 ==\n";
        let doc = Document::parse(src);
        assert_eq!(
            kinds(&doc),
            vec![
                BlockKind::Heading(2),
                BlockKind::BoldLead,
                BlockKind::ListItem,
                BlockKind::ListItem,
                BlockKind::Heading(2),
            ]
        );
        assert_eq!(doc.blocks()[4].text, "위키 소제목");
        assert_eq!(doc.slice(doc.blocks()[4].span), "== 위키 소제목 ==");
        assert_eq!(
            doc.slice(doc.blocks()[1].span).trim_end(),
            "**요점.** 설명입니다."
        );
        // 문단 중간의 볼드는 볼드 머리가 아니다.
        assert_eq!(
            kinds(&Document::parse("앞말 **볼드** 뒷말")),
            vec![BlockKind::Paragraph]
        );
    }

    #[test]
    fn sentences_map_back_to_source() {
        let src = "첫 문장입니다. 둘째는 `code` 포함이고\n줄이 바뀝니다! 버전 1.2는 그대로";
        let doc = Document::parse(src);
        let b = &doc.blocks()[0];
        let got: Vec<&str> = b.sentences.iter().map(|s| doc.slice(s.span)).collect();
        assert_eq!(
            got,
            vec![
                "첫 문장입니다.",
                "둘째는 `code` 포함이고\n줄이 바뀝니다!",
                "버전 1.2는 그대로"
            ]
        );
    }

    #[test]
    fn utf16_offsets() {
        let doc = Document::parse("🙂 가");
        assert_eq!(doc.utf16_offset("🙂 ".len()), 3);
    }
}
