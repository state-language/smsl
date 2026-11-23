use super::{
    printer::Doc,
    tree::{Body, Item},
};

pub(super) fn document<'a>(items: &'a [Item<'a>]) -> Doc<'a> {
    let memory = items
        .iter()
        .find(|item| !item.is_comment())
        .is_some_and(|item| item.text() == "memory");
    body(items, memory, true)
}

fn body<'a>(items: &'a [Item<'a>], memory: bool, root: bool) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut start = 0;
    let mut after_header = false;
    while start < items.len() {
        if !docs.is_empty() {
            docs.push(if items[start].blank || after_header {
                Doc::BlankLine
            } else {
                Doc::HardLine
            });
        }
        if items[start].is_comment() {
            docs.push(Doc::Text(items[start].text()));
            after_header = false;
            start += 1;
            continue;
        }
        let mut end = start + 1;
        if !items[start].is_comment() {
            while end <= items.len() {
                let last = &items[end - 1];
                let import = matches!(items[start].text(), "import" | "export");
                let block = matches!(&last.body, Body::Delimited { open: "{", .. })
                    && !import
                    && items[end..]
                        .iter()
                        .find(|item| !item.is_comment())
                        .is_none_or(|item| item.text() != ";");
                if last.text() == ";" || block || end == items.len() {
                    break;
                }
                end += 1;
            }
            while end < items.len() && items[end].inline_comment() {
                end += 1;
            }
        }
        let trailing = items[start..end]
            .iter()
            .rposition(|item| !item.inline_comment())
            .map_or(start, |index| start + index + 1);
        docs.push(flow(&items[start..trailing], memory, 0));
        for item in &items[trailing..end] {
            docs.extend([Doc::Text(" "), Doc::Text(item.text())]);
        }
        after_header = root && items[start].text() == "smsl";
        start = end;
    }
    Doc::Concat(docs)
}

fn flow<'a>(items: &'a [Item<'a>], memory: bool, level: usize) -> Doc<'a> {
    if level == 3 {
        return atoms(items, memory);
    }
    let mut starts = vec![0];
    for (index, item) in items.iter().enumerate().skip(1) {
        if items[index - 1].text() == "." || item.is_comment() {
            continue;
        }
        let split = match level {
            0 => matches!(item.text(), "when" | "set" | "emit" | "ensure" | "outcomes"),
            1 => matches!(item.text(), "and" | "or"),
            _ => matches!(
                item.text(),
                "==" | "!=" | "<" | "<=" | ">" | ">=" | "+" | "-" | "*" | "/" | "%"
            ),
        };
        if split {
            starts.push(index);
        }
    }
    starts.push(items.len());
    let mut pieces = starts
        .windows(2)
        .map(|span| flow(&items[span[0]..span[1]], memory, level + 1));
    let first = pieces.next().expect("at least one span");
    let rest = pieces.flat_map(|piece| [Doc::Line(" "), piece]).collect();
    Doc::Concat(vec![first, Doc::Concat(rest).nest()]).group()
}

fn atoms<'a>(items: &'a [Item<'a>], memory: bool) -> Doc<'a> {
    let mut docs = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let previous = index.checked_sub(1).map(|index| &items[index]);
        if let Body::Comment { text, inline } = &item.body {
            if previous.is_some() {
                docs.push(if *inline {
                    Doc::Text(" ")
                } else {
                    Doc::HardLine
                });
            }
            docs.extend([Doc::Text(text), Doc::HardLine]);
            continue;
        }
        if let Some(previous) = previous.filter(|previous| !previous.is_comment())
            && space(previous, item)
        {
            docs.push(Doc::Text(" "));
        }
        docs.push(match &item.body {
            Body::Token(text, _) => Doc::Text(text),
            Body::Delimited { open, items } => {
                let import =
                    previous.is_some_and(|item| matches!(item.text(), "import" | "export"));
                delimited(open, items, memory, import)
            }
            Body::Comment { .. } => unreachable!(),
        });
    }
    Doc::Concat(docs)
}

fn space(previous: &Item<'_>, next: &Item<'_>) -> bool {
    if matches!(next.text(), ";" | "," | ":" | "." | ".." | "?")
        || matches!(previous.text(), "." | "..")
        || next.text() == "[" && previous.text() == "uint"
    {
        return false;
    }
    if next.text() == "(" {
        return matches!(
            previous.text(),
            "not"
                | "and"
                | "or"
                | "when"
                | "ensure"
                | "activate"
                | "="
                | "=="
                | "!="
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "<"
                | ">"
                | "<="
                | ">="
        );
    }
    true
}

fn delimited<'a>(open: &'a str, items: &'a [Item<'a>], memory: bool, import: bool) -> Doc<'a> {
    let close = match open {
        "{" => "}",
        "[" => "]",
        "(" => ")",
        _ => unreachable!(),
    };
    if items.is_empty() {
        return Doc::Concat(vec![Doc::Text(open), Doc::Text(close)]);
    }
    if open == "{" && !memory && !import {
        let mut content = vec![Doc::HardLine, body(items, memory, false)];
        if items[0].inline_comment() {
            content = vec![Doc::Text(" "), body(items, memory, false)];
        }
        return Doc::Concat(vec![
            Doc::Text(open),
            Doc::Concat(content).nest(),
            Doc::HardLine,
            Doc::Text(close),
        ]);
    }
    let edge = if import { " " } else { "" };
    let mut content = vec![Doc::Line(edge)];
    let mut start = 0;
    for (index, item) in items.iter().enumerate() {
        if item.text() == "," {
            content.push(flow(&items[start..=index], memory, 0));
            start = index + 1;
            if let Some(next) = items.get(start) {
                content.push(if next.inline_comment() {
                    Doc::Text(" ")
                } else {
                    Doc::Line(" ")
                });
            }
        }
    }
    if start < items.len() {
        content.push(flow(&items[start..], memory, 0));
    }
    Doc::Concat(vec![
        Doc::Text(open),
        Doc::Concat(content).nest(),
        Doc::Line(edge),
        Doc::Text(close),
    ])
    .group()
}
