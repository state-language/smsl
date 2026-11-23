use super::format;
use crate::source::lexer::{Kind, tokenize};

fn spelling(source: &str) -> Vec<&str> {
    tokenize(source)
        .unwrap()
        .iter()
        .filter(|token| token.kind != Kind::End)
        .map(|token| &source[token.location.offset..token.end])
        .collect()
}

#[test]
fn comments_between_every_token_preserve_spelling_and_do_not_drift() {
    for source in [
        r#"smsl "0.2"; import { A as B, C } from "missing.smsl";
export { B }; branch T(value: string) {
fact n:uint[0..10]=0; fact ready:[false,true,]=false;
state S(n:0,) {go -> S set(n=self.n+1,);}
operation go when (self.n<10 and not self.ready) set(n=self.n+1) ensure self.n<=10;
action act(value:string) when args.value=="// \u0020 🤖" outcomes(done,failed);
} instance a:T(value="escaped\\\"text");"#,
        r#"smsl "0.2"; system sample { import "missing.smsl";
component planner: "symbolic-planner" { block plan: symbolic.plan {
provider "impl"; config { "a/b" = [1, { enabled = true; }]; empty={}; };
} } }"#,
        r#"memory world "1" {value: [unknown, null, "unicode 🤖", {nested: true}]}"#,
    ] {
        let expected = spelling(source);
        for ending in ["\n", "\r\n", "\r"] {
            let mut commented = format!("// first{ending}");
            let mut comments = vec!["// first".to_owned()];
            for (index, token) in expected.iter().enumerate() {
                let comment = format!("// token {index} 🤖  ");
                commented.push_str(&format!("{token} {comment}{ending}"));
                comments.push(comment);
            }
            let formatted = format(&commented).unwrap();
            assert_eq!(spelling(&formatted), expected);
            let mut position = 0;
            for comment in comments {
                let found = formatted[position..].find(&comment).unwrap();
                position += found + comment.len();
            }
            assert_eq!(format(&formatted).unwrap(), formatted, "{formatted}");
        }
    }
}

#[test]
fn delimiters_ranges_and_quoted_versions_remain_tokens() {
    for source in [
        r#"smsl "0.\u0032"; system s {}"#,
        r#"smsl "0.2"; branch "a" { fact n:uint[0..100]=0; operation inc set(n=(self.n+1)*2); }"#,
    ] {
        let formatted = format(source).unwrap();
        assert_eq!(spelling(&formatted), spelling(source));
    }
}
