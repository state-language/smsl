use super::super::lexer::{Kind, Token};

pub(super) struct Item<'a> {
    pub body: Body<'a>,
    pub blank: bool,
}

pub(super) enum Body<'a> {
    Token(&'a str, &'a Kind),
    Comment { text: &'a str, inline: bool },
    Delimited { open: &'a str, items: Vec<Item<'a>> },
}

impl Item<'_> {
    pub fn text(&self) -> &str {
        match &self.body {
            Body::Token(text, _) => text,
            Body::Comment { text, .. } => text,
            Body::Delimited { open, .. } => open,
        }
    }

    pub fn is_comment(&self) -> bool {
        matches!(self.body, Body::Comment { .. })
    }

    pub fn inline_comment(&self) -> bool {
        matches!(self.body, Body::Comment { inline: true, .. })
    }
}

pub(super) fn read<'a>(source: &'a str, tokens: &'a [Token]) -> Vec<Item<'a>> {
    let mut flat = Vec::new();
    let mut end = 0;
    for token in tokens {
        let gap = &source[end..token.location.offset];
        let mut cursor = 0;
        let mut lines = 0;
        let bytes = gap.as_bytes();
        while cursor < gap.len() {
            match bytes[cursor] {
                b'/' => {
                    let finish = gap[cursor..]
                        .find(['\r', '\n'])
                        .map_or(gap.len(), |offset| cursor + offset);
                    flat.push(Item {
                        body: Body::Comment {
                            text: &gap[cursor..finish],
                            inline: lines == 0 && end > 0,
                        },
                        blank: lines > 1,
                    });
                    cursor = finish;
                    lines = 0;
                }
                b'\r' => {
                    lines += 1;
                    cursor += 1 + usize::from(bytes.get(cursor + 1) == Some(&b'\n'));
                }
                b'\n' => {
                    lines += 1;
                    cursor += 1;
                }
                _ => cursor += 1,
            }
        }
        if token.kind != Kind::End {
            flat.push(Item {
                body: Body::Token(&source[token.location.offset..token.end], &token.kind),
                blank: lines > 1,
            });
        }
        end = token.end;
    }
    nest(&mut flat.into_iter())
}

fn nest<'a>(input: &mut impl Iterator<Item = Item<'a>>) -> Vec<Item<'a>> {
    let mut items = Vec::new();
    while let Some(mut item) = input.next() {
        match item.body {
            Body::Token(_, Kind::CloseBranch | Kind::CloseList | Kind::CloseMap) => break,
            Body::Token(open, Kind::OpenBranch | Kind::OpenList | Kind::OpenMap) => {
                item.body = Body::Delimited {
                    open,
                    items: nest(input),
                };
            }
            _ => {}
        }
        items.push(item);
    }
    items
}
