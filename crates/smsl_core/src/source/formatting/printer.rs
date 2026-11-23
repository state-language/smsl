pub(super) enum Doc<'a> {
    Text(&'a str),
    Line(&'a str),
    HardLine,
    BlankLine,
    Concat(Vec<Doc<'a>>),
    Nest(Box<Doc<'a>>),
    Group(Box<Doc<'a>>),
}

impl<'a> Doc<'a> {
    pub fn group(self) -> Self {
        Self::Group(Box::new(self))
    }

    pub fn nest(self) -> Self {
        Self::Nest(Box::new(self))
    }
}

pub(super) fn render(doc: &Doc<'_>) -> String {
    let mut output = String::new();
    let mut column = 0;
    let mut pending = vec![(0, false, doc)];
    while let Some((indent, flat, doc)) = pending.pop() {
        match doc {
            Doc::Text(text) | Doc::Line(text) if flat || matches!(doc, Doc::Text(_)) => {
                if text.is_empty() {
                    continue;
                }
                if column == 0 {
                    output.extend(std::iter::repeat_n(' ', indent));
                    column = indent;
                }
                output.push_str(text);
                column += text.chars().count();
            }
            Doc::Line(_) | Doc::HardLine | Doc::BlankLine => {
                if !output.ends_with('\n') && !output.is_empty() {
                    output.push('\n');
                }
                if matches!(doc, Doc::BlankLine) && !output.ends_with("\n\n") {
                    output.push('\n');
                }
                column = 0;
            }
            Doc::Concat(docs) => {
                pending.extend(docs.iter().rev().map(|doc| (indent, flat, doc)));
            }
            Doc::Nest(doc) => pending.push((indent + 4, flat, doc)),
            Doc::Group(doc) => {
                let available = 100usize.saturating_sub(if column == 0 { indent } else { column });
                pending.push((indent, flat || fits(doc, available), doc));
            }
            Doc::Text(_) => unreachable!(),
        }
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn fits(doc: &Doc<'_>, mut remaining: usize) -> bool {
    let mut pending = vec![doc];
    while let Some(doc) = pending.pop() {
        match doc {
            Doc::Text(text) | Doc::Line(text) => {
                let width = text.chars().count();
                if width > remaining {
                    return false;
                }
                remaining -= width;
            }
            Doc::HardLine | Doc::BlankLine => return false,
            Doc::Concat(docs) => pending.extend(docs.iter().rev()),
            Doc::Nest(doc) | Doc::Group(doc) => pending.push(doc),
        }
    }
    true
}
