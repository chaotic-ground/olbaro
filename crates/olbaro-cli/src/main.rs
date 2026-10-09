//! `olbaro [--enable 묶음|규칙]... [--disable 묶음|규칙]... [--count] [--list-rules] [파일...]`
//!
//! 파일을 주지 않으면 표준 입력을 읽는다. 진단이 하나라도 있으면 종료 코드 1.

use std::collections::BTreeMap;
use std::io::Read;
use std::process::ExitCode;

use olbaro_core::{Config, Document, Linter, Severity};

const USAGE: &str = "사용법: olbaro [--enable 묶음|규칙]... [--disable 묶음|규칙]... [--count] [--list-rules] [파일...]";

fn main() -> ExitCode {
    let mut config = Config::new();
    let mut count = false;
    let mut list = false;
    let mut files = Vec::new();
    let mut toggles = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--enable" | "--disable" => match args.next() {
                Some(name) => toggles.push((name, arg == "--enable")),
                None => return usage(),
            },
            "--count" => count = true,
            "--list-rules" => list = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if arg.starts_with("--") => return usage(),
            _ => files.push(arg),
        }
    }

    let rules = olbaro_rules::all();
    let groups: Vec<&str> = rules.iter().map(|r| r.group()).collect();
    for (name, on) in toggles {
        // 이름이 묶음이면 묶음째, 아니면 규칙 하나를 켜고 끈다.
        if groups.contains(&name.as_str()) {
            config.set_group(name, on);
        } else if rules.iter().any(|r| r.id() == name) {
            config.set_rule(name, on);
        } else {
            eprintln!("모르는 규칙이나 묶음: {name}");
            return ExitCode::from(2);
        }
    }
    let linter = Linter::new(rules, config);

    if list {
        for (rule, enabled) in linter.rules() {
            println!(
                "{} {:<28} {}",
                if enabled { "켬" } else { "끔" },
                rule.id(),
                rule.description()
            );
        }
        return ExitCode::SUCCESS;
    }

    if files.is_empty() {
        files.push("-".into());
    }
    let mut found = false;
    for path in &files {
        let source = match read(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::from(2);
            }
        };
        let doc = Document::parse(&source);
        let diags = linter.lint_document(&doc);
        found |= !diags.is_empty();
        if count {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for d in &diags {
                *counts.entry(d.rule).or_default() += 1;
            }
            let mut counts: Vec<_> = counts.into_iter().collect();
            counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            for (rule, n) in counts {
                println!("{n:4}  {rule}");
            }
            continue;
        }
        for d in diags {
            let (line, col) = line_col(&source, d.span.start);
            let excerpt: String = doc
                .slice(d.span)
                .replace('\n', " ")
                .chars()
                .take(40)
                .collect();
            let level = match d.severity {
                Severity::Error => "오류",
                Severity::Warning => "경고",
                Severity::Hint => "안내",
            };
            print!(
                "{path}:{line}:{col}: {level} [{}] {}: «{excerpt}»",
                d.rule, d.message
            );
            // 바꿀 글이 있으면 그것을, 없으면 조언을 보인다.
            match (d.replacements.first(), d.suggestion) {
                (Some(r), _) => println!(" → «{r}»"),
                (None, Some(s)) => println!(" → {s}"),
                (None, None) => println!(),
            }
        }
    }
    if found {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn read(path: &str) -> std::io::Result<String> {
    if path == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        Ok(s)
    } else {
        std::fs::read_to_string(path)
    }
}

/// 1부터 세는 줄과 칸(글자 단위).
fn line_col(source: &str, byte: usize) -> (usize, usize) {
    let before = &source[..byte];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, col)
}
