use crate::error::ParseError;
use std::iter::Peekable;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Alt,
    SetBegin,
    SetEnd,
    BackSlash,
    Star,
    Question,
    Dash,
    Quote,
    Plus,
    Any,
    Comma,
    Equals,
    Colon,
    Not,
    GreaterThan,
    Tick,
    BackTick,
    RepeatBegin,
    RepeatEnd,
    GroupBegin,
    GroupEnd,
    StartAnchor,
    EndAnchor,
    Number(u8),
    Character(u8),
}

const INFINITE: i32 = 1 << 16;
const REPEAT_MAX: i32 = INFINITE - 1;
const NON_GREEDY: i32 = 1 << 17;
const PROSSESSIVE: i32 = 1 << 18;
const CLEAR_FLAGS: i32 = NON_GREEDY - 1;

fn char_to_token(ch: &u8) -> Token {
    match ch {
        b'|' => Token::Alt,
        b'[' => Token::SetBegin,
        b']' => Token::SetEnd,
        b'\'' => Token::Tick,
        b'*' => Token::Star,
        b'\\' => Token::BackSlash,
        b'?' => Token::Question,
        b'-' => Token::Dash,
        b'"' => Token::Quote,
        b'+' => Token::Plus,
        b'.' => Token::Any,
        b',' => Token::Comma,
        b'`' => Token::BackTick,
        b'{' => Token::RepeatBegin,
        b':' => Token::Colon,
        b'=' => Token::Equals,
        b'!' => Token::Not,
        b'>' => Token::GreaterThan,
        b'}' => Token::RepeatEnd,
        b'(' => Token::GroupBegin,
        b')' => Token::GroupEnd,
        b'^' => Token::StartAnchor,
        b'$' => Token::EndAnchor,
        n @ 48..=57 => Token::Number(*n),
        ch => Token::Character(*ch),
    }
}

fn token_to_char(token: &Token) -> u8 {
    match token {
        Token::Alt => b'|',
        Token::SetBegin => b'[',
        Token::SetEnd => b']',
        Token::Tick => b'\'',
        Token::Star => b'*',
        Token::BackSlash => b'\\',
        Token::Question => b'?',
        Token::Dash => b'-',
        Token::Quote => b'"',
        Token::Plus => b'+',
        Token::Any => b'.',
        Token::Comma => b',',
        Token::BackTick => b'`',
        Token::RepeatBegin => b'{',
        Token::Colon => b':',
        Token::Equals => b'=',
        Token::Not => b'!',
        Token::GreaterThan => b'>',
        Token::RepeatEnd => b'}',
        Token::GroupBegin => b'(',
        Token::GroupEnd => b')',
        Token::StartAnchor => b'^',
        Token::EndAnchor => b'$',
        Token::Number(n) => *n,
        Token::Character(ch) => *ch,
    }
}

fn is_infinite(node: &Node) -> bool {
    match node {
        Node::Repeat(Token::Question) => (1 & INFINITE) != 0,
        _ => false,
    }
}

fn is_non_greedy(node: &Node) -> bool {
    match node {
        Node::Repeat(Token::Plus) => (1 & NON_GREEDY) != 0,
        _ => false,
    }
}

fn is_possessive(node: &Node) -> bool {
    match node {
        Node::Repeat(Token::Plus) => (1 & PROSSESSIVE) != 0,
        _ => false,
    }
}

fn can_repeat(node: &Node) -> bool {
    match node {
        Node::Repeat(Token::Question) => (0 & NON_GREEDY | PROSSESSIVE) != 0,
        Node::Repeat(Token::Star) => (0 & NON_GREEDY | PROSSESSIVE) != 0,
        Node::Repeat(Token::Plus) => (1 & NON_GREEDY | PROSSESSIVE) != 0,
        _ => false,
    }
}

struct ParseData {
    ends: Vec<u8>,
    i: usize,
    reference: i32,
}

#[derive(Clone, Debug)]
pub enum Node {
    Edge,
    Text(u8),
    Charset(Vec<u8>),
    ExcludeCharset(Vec<u8>),
    Repeat(Token),
    FixedRepeat(i32, i32),
    Seq(Vec<Node>),
    Set(Vec<Node>),
    Group(Vec<Node>),
    Ref(u8),
    Select(Vec<Node>),
}

fn process_repeat<I>(mut piter: Peekable<I>, seq: &mut Vec<Node>) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    let mut min: i32 = 0;
    let mut max: i32 = INFINITE;
    while let (Some(first), Some(peek)) = (piter.next(), piter.peek()) {
        dbg!(&first);
        match (first, peek) {
            (Token::Number(n), Token::Comma) => min = n as i32,
            (Token::Number(n), Token::RepeatEnd) => seq.push(Node::FixedRepeat(n as i32, n as i32)),
            (Token::Comma, Token::Number(n)) => max = *n as i32,
            (_, Token::RepeatEnd) => seq.push(Node::FixedRepeat(min, max)),
            _ => break,
        }
    }
    piter
}

fn process_group<I>(mut piter: Peekable<I>, seq: &mut Vec<Node>) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    let mut group = Vec::new();
    while let Some(ref n) = piter.peek() {
        dbg!(&n);
        match n {
            Token::Question => {
                piter.next();
                if let Some(p) = piter.peek() {
                    dbg!(&p);
                    match p {
                        Token::Equals | Token::Colon | Token::Not | Token::GreaterThan => {
                            piter.next();
                        }
                        _ => (),
                    }
                }
            }
            _ => break,
        }
    }
    piter = process_seq(piter, &mut group);
    seq.push(Node::Group(group));
    piter
}

fn append_node(parent: &mut Option<Node>, cur: Option<Node>) {
    if parent.is_none() {
        *parent = Some(Node::Seq(vec![cur.unwrap()]))
    } else {

    }
}

fn process_range<I>(mut piter: Peekable<I>, end: &mut u8) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    while let Some(ref n) = piter.next() {
        dbg!(&n);
        match n {
            Token::Character(ch) => *end = *ch,
            Token::Number(n) => *end = *n,
            _ => break,
        }
    }
    piter
}

fn process_set<I>(mut piter: Peekable<I>, seq: &mut Vec<Node>) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    let mut set = Vec::new();
    let mut prev: u8 = 0;
    let mut begin = true;
    while let Some(ref n) = piter.next() {
        dbg!(&n);
        match n {
            Token::StartAnchor if begin == true => {
                begin = false;
                set.push(Node::Edge)
            }
            Token::Dash if prev != 0 => {
                let mut end = 0;
                piter = process_range(piter, &mut end);
                set.push(Node::Charset((prev..=end).collect()));
                prev = 0;
            }
            Token::SetEnd => {
                break;
            }
            Token::BackSlash => {
                piter = process_slash(piter, &mut set);
            }
            tok => prev = token_to_char(tok),
        }
    }
    seq.push(Node::Set(set));
    piter
}

fn process_select<I>(mut piter: Peekable<I>, seq: &mut Vec<Node>) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    let mut select = Vec::new();
    piter = process_seq(piter, &mut select);
    seq.push(Node::Select(select));
    piter
}

fn process_slash<I>(mut piter: Peekable<I>, seq: &mut Vec<Node>) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    while let Some(ref n) = piter.peek() {
        dbg!(&n);
        match n {
            Token::BackSlash => seq.push(Node::Charset([b'\\'].to_vec())),
            Token::Character(ch) if *ch == b'n' => seq.push(Node::Charset([b'\n'].to_vec())),
            Token::Character(ch) if *ch == b'r' => seq.push(Node::Charset([b'\r'].to_vec())),
            Token::Character(ch) if *ch == b't' => seq.push(Node::Charset([b'\t'].to_vec())),
            Token::Character(ch) if *ch == b'd' => seq.push(Node::Charset((b'0'..=b'9').collect())),
            Token::Character(ch) if *ch == b'D' => {
                seq.push(Node::ExcludeCharset((b'0'..=b'9').collect()))
            }
            Token::Character(ch) if *ch == b's' => seq.push(Node::Charset([b'\t', b' '].to_vec())),
            Token::Character(ch) if *ch == b'w' => {
                let mut v = (b'A'..=b'Z')
                    .take(26)
                    .chain((b'a'..=b'z').take(26))
                    .chain((b'0'..=b'9').take(10))
                    .collect::<Vec<_>>();
                v.push(b'_');
                seq.push(Node::Charset(v));
            }
            Token::Character(ch) if *ch == b'W' => {
                let mut v = (b'A'..=b'Z')
                    .take(26)
                    .chain((b'a'..=b'z').take(26))
                    .chain((b'0'..=b'9').take(10))
                    .collect::<Vec<_>>();
                v.push(b'_');
                seq.push(Node::ExcludeCharset(v));
            }
            Token::Number(n) => seq.push(Node::Ref(*n)),
            Token::Character(ch) => seq.push(Node::Charset(vec![*ch])),
            _ => break, //panic!("Unexpected token at slash {:?}", n),
        }
        piter.next();
    }
    piter
}

fn process_seq<I>(mut piter: Peekable<I>, seq: &mut Vec<Node>) -> Peekable<I>
where
    I: Iterator<Item = Token>,
{
    let mut seq_p = Vec::new();
    while let Some(ref n) = piter.next() {
        dbg!(&n);
        match n {
            Token::StartAnchor => seq_p.push(Node::Edge),
            Token::SetBegin => piter = process_set(piter, &mut seq_p),
            Token::BackSlash => piter = process_slash(piter, &mut seq_p),
            Token::GroupBegin => piter = process_group(piter, &mut seq_p),
            Token::Alt => piter = process_select(piter, &mut seq_p),
            Token::RepeatBegin if !seq_p.is_empty() => piter = process_repeat(piter, &mut seq_p),
            c @ Token::Question | c @ Token::Plus | c @ Token::Star if !seq_p.is_empty() => {
                seq_p.push(Node::Repeat(c.clone()))
            }
            Token::Any => {
                seq_p.push(Node::Charset((32u8..126u8).take(94).collect()));
            }
            Token::GroupEnd => (),
            Token::RepeatEnd => (),
            Token::EndAnchor => (),
            Token::SetEnd => (),
            Token::Dash => seq_p.push(Node::Text(b'-')),
            Token::Quote => seq_p.push(Node::Text(b'"')),
            Token::Character(ch) => seq_p.push(Node::Text(*ch)),
            Token::Number(n) => seq_p.push(Node::Text(*n)),
            Token::Colon => seq_p.push(Node::Text(b':')),
            _ => panic!("Unexpected pattern in sequence {:?}", n),
        }
    }
    seq.extend(seq_p);
    piter
}

pub fn parse(re: Vec<u8>) -> Result<Vec<Node>, ParseError> {
    let re = pre_parse(re);
    // Lex the whole regex
    let mut iter_chars = re.iter().map(|ch| char_to_token(&ch)).peekable();
    let mut seq = Vec::new();
    let top = process_seq(iter_chars, &mut seq);
    Ok(seq)
}

fn pre_parse(mut s: Vec<u8>) -> Vec<u8> {
    const BEGIN: &[u8] = &[b'^'];
    const END: &[u8] = &[b'$'];
    if let Some(&b) = s.first() {
        if b != b'^' {
            s.splice(0..0, BEGIN.iter().cloned());
        }
    }
    if let Some(&b) = s.last() {
        if b != b'$' {
            s.splice(s.len() - 1..s.len() - 1, END.iter().cloned());
        }
    }
    dbg!(&s);
    s
}

#[cfg(test)]
mod tests {

    #[test]
    fn parse_1() {
        println!("{:?}", super::parse(b"[A-Z]{1,2}".to_vec()));
    }

    #[test]
    fn parse_2() {
        println!("{:?}", super::parse(b"[\\w]{1,2}".to_vec()));
    }

    #[test]
    fn parse_3() {
        println!(
            "{:?}",
            super::parse(b"\\w+([-+.]\\w+)*@\\w+([-.]\\w+)*\\.\\w+([-.]\\w+)*".to_vec())
        );
    }

    #[test]
    fn parse_4() {
        println!(
            "{:?}",
            super::parse(r"^[\w\.\']{2,}([\s][\w\.\']{2,})+$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_5() {
        println!(
            "{:?}",
            super::parse(
                r"^([a-z][a-z0-9\-]+(\.|\-*\.))+[a-z]{2,6}$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_6() {
        println!(
            "{:?}",
            super::parse(
                r"^(\d|[1-9]\d|1\d\d|2[0-4]\d|25[0-5])\.(\d|[1-9]\d|1\d\d|2[0-4]\d|25[0-5]){3}$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_7() {
        println!(
            "{:?}",
            super::parse(
                r"^[\_]*([a-z0-9]+(\.|\_*)?)+@([a-z][a-z0-9\-]+(\.|\-*\.))+[a-z]{2,6}$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_8() {
        println!("{:?}",super::parse(r"^([1-9]|0[1-9]|[12][0-9]|3[01])\D([1-9]|0[1-9]|1[012])\D(19[0-9][0-9]|20[0-9][0-9])$".as_bytes().to_vec()));
    }
    #[test]
    fn parse_9() {
        println!(
            "{:?}",
            super::parse(
                r"^-?([1-9]\d*\.\d*|0\.\d*[1-9]\d*|0?\.0+|0)$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_10() {
        println!(
            "{:?}",
            super::parse(
                r"^[1-9]\d*\.\d*|0\.\d*[1-9]\d*|0?\.0+|0$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_11() {
        println!(
            "{:?}",
            super::parse(
                r"^(-([1-9]\d*\.\d*|0\.\d*[1-9]\d*))|0?\.0+|0$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_12() {
        println!(
            "{:?}",
            super::parse(r"^<([a-z]+)[^<a-z]+?(?:>(.*)<\/\1>)$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_13() {
        println!(
            "{:?}",
            super::parse(
                r"^<([a-z]+)[^<a-z]+?(?:>(.*)<\/\1>|\s+\/>)$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_14() {
        println!(
            "{:?}",
            super::parse(
                r"^<([a-z]+)[^<a-z]+?(?:>(.*)<\/\\>|\s+\/>)$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_15() {
        println!(
            "{:?}",
            super::parse(r"(|)((()|(|))|())".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_16() {
        println!("{:?}", super::parse(r"(c((w)|e+)\?)*".as_bytes().to_vec()));
    }
    #[test]
    fn parse_17() {
        println!(
            "{:?}",
            super::parse(r"(?:c(?:(?:w)|e+))*".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_18() {
        println!(
            "{:?}",
            super::parse(r"a{2,2}|(c((w)|e+)\?)*[^^bd?\]]?$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_19() {
        println!("{:?}",super::parse(r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$".as_bytes().to_vec()));
    }
    #[test]
    fn parse_20() {
        println!(
            "{:?}",
            super::parse(
                r"^(https?:\/\/)?([\da-z\.-]+)\.([a-z\.]{2,6})([\/\w \.-]*)*\/?$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_21() {
        println!("{:?}", super::parse(r"[^\D12]+".as_bytes().to_vec()));
    }
    #[test]
    fn parse_22() {
        println!(
            "{:?}",
            super::parse(
                r"^([a-z0-9_\.-]+)@([\da-z\.-]+)\.([a-z\.]{2,6})$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_23() {
        println!(
            "{:?}",
            super::parse(r"^#?([a-f0-9]{6}|[a-f0-9]{3})$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_24() {
        println!(
            "{:?}",
            super::parse(r"^[a-z0-9_-]{6,18}$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_25() {
        println!(
            "{:?}",
            super::parse(r"^\w[-\w\d]\d{8}$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_26() {
        println!("{:?}", super::parse(r".?[\W]{3,16}$".as_bytes().to_vec()));
    }
    #[test]
    fn parse_27() {
        println!(
            "{:?}",
            super::parse(r"^.?[.-.d \-~a-z0-9_-]{3,16}$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_28() {
        println!(
            "{:?}",
            super::parse(
                r"\(\(\(ab\)*c\)*d\)\(ef\)*\(gh\)\{2\}\(ij\)*\(kl\)*\(mn\)*\(op\)*\(qr\)*"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_29() {
        println!(
            "{:?}",
            super::parse(r"(c((a)|e+)\?)*[^^bd?\]]?$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_30() {
        println!(
            "{:?}",
            super::parse(r"^#?([a-f0-9]{6}|[a-f0-9]{3})$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_31() {
        println!("{:?}", super::parse(r"(a)\1+?\\+?".as_bytes().to_vec()));
    }
    #[test]
    fn parse_32() {
        println!(
            "{:?}",
            super::parse("\"(?:[^\"\\]++|\\.)*+\"".as_bytes().to_vec())
        );
    }

    #[test]
    fn parse_33() {
        println!(
            "{:?}",
            super::parse(r"^\D?(\d{3})\D?\D?(\d{3})\D?(\d{4})$".as_bytes().to_vec())
        );
    }
    #[test]
    fn parse_34() {
        println!("{:?}",super::parse(r"^(http|https|ftp)\://[a-zA-Z0-9\-\.]+\.[a-zA-Z]{2,3}(:[a-zA-Z0-9]*)?/?([a-zA-Z0-9\-\._\?\,\'/\\\+&%\$#\=~])*$".as_bytes().to_vec()));
    }
    #[test]
    fn parse_35() {
        println!(
            "{:?}",
            super::parse(
                r"^(\d{5}-\d{4}|\d{5}|\d{9})$|^([a-zA-Z]\d[a-zA-Z] \d[a-zA-Z]\d)$"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_36() {
        println!("{:?}",super::parse(r"\w+([-+.]\w+)*@\w+([-.]\w+)*\.\w+([-.]\w+)*([,;]\s*\w+([-+.]\w+)*@\w+([-.]\w+)*\.\w+([-.]\w+)*)*".as_bytes().to_vec()));
    }
    #[test]
    fn parse_37() {
        println!("{:?}",super::parse(r"((?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]\d|[1-9]))".as_bytes().to_vec()));
    }
    #[test]
    fn parse_38() {
        println!(
            "{:?}",
            super::parse(
                r"(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.\1){3}"
                    .as_bytes()
                    .to_vec()
            )
        );
    }
    #[test]
    fn parse_39() {
        let regex = b"(a-z|A-Z|:;+)[cd]{2}\\1";
        println!("{:?}", super::parse(b"(a-z|A-Z|:;+)[cd]{2}\\1".to_vec()));
    }

}
