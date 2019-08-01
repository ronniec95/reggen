use crate::error::ParseError;
use std::iter::Peekable;

const INFINITE: i32 = 1 << 16;
const REPEAT_MAX: i32 = INFINITE - 1;
const NON_GREEDY: i32 = 1 << 17;
const PROSSESSIVE: i32 = 1 << 18;
const CLEAR_FLAGS: i32 = NON_GREEDY - 1;

#[derive(Clone, PartialEq, Debug)]
struct Repeat {
    min: i32,
    max: i32,
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Edge(bool),
    Text(u8, Option<Repeat>),
    Charset(Vec<u8>, bool, Option<Repeat>),
    Seq(Vec<Node>, Option<Repeat>),
    Group(Vec<Node>, usize, Option<Repeat>),
    Select(Vec<Node>, Option<Repeat>),
    Ref(usize),
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

fn process_int<'a, I>(iter: &mut Peekable<I>, num: &mut i32) -> u8
where
    I: Iterator<Item = &'a u8>,
{
    let mut number_str = Vec::with_capacity(6);
    let mut ret_ch = 0;
    while let Some(ch) = iter.next() {
        dbg!(&ch);
        match ch {
            b'0'..=b'9' => number_str.push(*ch),
            _ => {
                ret_ch = *ch;
                break;
            }
        }
    }
    if !number_str.is_empty() {
        *num = String::from_utf8_lossy(number_str.as_slice())
            .parse::<i32>()
            .unwrap();
    }
    ret_ch
}

fn process_defined_repeat<'a, 'b, I>(iter: &mut Peekable<I>, min: &'b mut i32, max: &'b mut i32)
where
    I: Iterator<Item = &'a u8>,
{
    if let Some(ch) = iter.peek() {
        match ch {
            b'0'..=b'9' => {
                let ch = process_int(iter, min);
                match ch {
                    b'}' => *max = *min,
                    b',' => {
                        iter.next();
                        let _ = process_int(iter, max);
                    }
                    _ => (),
                }
            }
            b',' => {
                iter.next();
                let _ = process_int(iter, max);
            }
            _ => (),
        }
    }
}

fn process_repeat<'a, I>(iter: &mut Peekable<I>) -> Option<(i32, i32)>
where
    I: Iterator<Item = &'a u8>,
{
    let mut min = 0;
    let mut max = 65536;
    if let Some(ch) = iter.peek() {
        match ch {
            b'?' => {
                iter.next();
                Some((0, 1))
            }
            b'*' => {
                iter.next();
                Some((0, max))
            }
            b'+' => {
                iter.next();
                Some((1, max))
            }
            b'{' => {
                iter.next();
                process_defined_repeat(iter, &mut min, &mut max);
                Some((min, max))
            }
            _ => None,
        }
    } else {
        None
    }
}

fn process_select<'a, I>(iter: &mut Peekable<I>, ends: &mut Vec<u8>) -> Node
where
    I: Iterator<Item = &'a u8>,
{
    ends.push(b'|');
    let mut select = Vec::new();
    while let Some(&ch) = iter.peek() {
        select.push(process_seq(iter, ends));
    }
    ends.pop();
    Node::Select(select, None)
}

fn process_range<'a, I>(iter: &mut Peekable<I>, to: &mut u8)
where
    I: Iterator<Item = &'a u8>,
{
    while let Some(ch) = iter.next() {
        if *ch != b']' {
            *to = *ch;
            break;
        }
    }
}

fn process_slash<'a, I>(iter: &mut Peekable<I>, back_ref: bool) -> Node
where
    I: Iterator<Item = &'a u8>,
{
    let ch = if let Some(slash) = iter.next() {
        dbg!(&slash);
        match slash {
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            _ => *slash,
        }
    } else {
        b'\\'
    };
    dbg!(&ch);
    match ch {
        b'd' => Node::Charset((b'0'..=b'9').collect(), true, None),
        b'D' => Node::Charset((b'0'..=b'9').collect(), false, None),
        b's' => Node::Charset(vec![b'\t', b' '], true, None),
        b'S' => Node::Charset(vec![b'\t', b' '], false, None),
        b'w' => {
            let mut charset = Vec::with_capacity(255);
            charset.extend_from_slice((b'A'..=b'Z').collect::<Vec<_>>().as_slice());
            charset.extend_from_slice((b'a'..=b'z').collect::<Vec<_>>().as_slice());
            charset.extend_from_slice((b'0'..=b'9').collect::<Vec<_>>().as_slice());
            charset.push(b'_');
            Node::Charset(charset, true, None)
        }
        b'W' => {
            let mut charset = Vec::with_capacity(255);
            charset.extend_from_slice((b'A'..=b'Z').collect::<Vec<_>>().as_slice());
            charset.extend_from_slice((b'a'..=b'z').collect::<Vec<_>>().as_slice());
            charset.extend_from_slice((b'0'..=b'9').collect::<Vec<_>>().as_slice());
            charset.push(b'_');
            Node::Charset(charset, false, None)
        }
        b'0'..=b'9' if back_ref => Node::Ref((ch - b'0') as usize),
        _ => Node::Text(ch, None),
    }
}

fn process_set<'a, I>(iter: &mut Peekable<I>, ends: &mut Vec<u8>) -> Node
where
    I: Iterator<Item = &'a u8>,
{
    let mut charset = Vec::with_capacity(255);
    let mut begin = true;
    let mut prev = 0;
    let mut exclude = true;
    ends.push(b']');
    while let Some(&ch) = iter.peek() {
        dbg!(&(*ch as char));
        dbg!(prev as char);

        match ch {
            b'^' if begin => {
                begin = false;
                exclude = false;
                iter.next();
                continue;
            }
            b'-' if prev > 0 => {
                iter.next();
                let mut to = 0;
                process_range(iter, &mut to);
                charset.append(&mut (prev..=to).collect());
                prev = 0;
                continue;
            }
            b'|' => {
                if prev > 0 {
                    charset.push(prev);
                }
                iter.next();
                continue;
            }
            _ => (),
        }
        if prev > 0 {
            charset.push(prev);
        }
        match ch {
            b']' => {
                ends.pop();
                iter.next();
                if prev > 0 {
                    charset.push(prev);
                }
                break;
            }
            b'\\' => {
                iter.next();
                let node = process_slash(iter, false);
                match node {
                    Node::Charset(mut v, _, _) => charset.append(&mut v),
                    _ => (),
                }
            }
            _ => (),
        }
        prev = *ch;
        iter.next();
    }
    if !charset.is_empty() {
        Node::Charset(charset, exclude, None)
    } else {
        Node::Text(b'[', None)
    }
}

fn is_sub_expr<'a, I>(iter: &mut Peekable<I>) -> u8
where
    I: Iterator<Item = &'a u8>,
{
    let mut is_subexp = false;
    let mut ch: u8 = 0;
    while let Some(n) = iter.peek() {
        match n {
            b'?' => is_subexp = true,
            b':' | b'=' | b'!' | b'>' if is_subexp => {
                iter.next();
                break;
            }
            _ => {
                ch = **n;
                break;
            }
        }
        iter.next();
    }
    ch
}

fn process_group<'a, I>(iter: &mut Peekable<I>, ends: &mut Vec<u8>) -> Node
where
    I: Iterator<Item = &'a u8>,
{
    let mut group = Vec::new();
    ends.push(b')');
    let mark = is_sub_expr(iter);
    group.push(process_seq(iter, ends));
    ends.pop();
    if !group.is_empty() {
        return Node::Group(group, mark as usize, None);
    }
    return Node::Text(b'(', None);
}

fn process_seq<'a, I>(iter: &mut Peekable<I>, ends: &mut Vec<u8>) -> Node
where
    I: Iterator<Item = &'a u8>,
{
    let mut seq = Vec::new();
    let mut node: Option<Node> = None;
    let mut begin = true;
    while let Some(&ch) = iter.peek() {
        dbg!(*ch as char);
        if begin {
            if *ch == b'^' {
                node = Some(Node::Edge(true));
            } else {
                begin = false;
            }
        }

        if ends.iter().any(|e| e == ch) {
            iter.next();
            break;
        }
        node = match ch {
            b'|' => {
                iter.next();
                Some(process_select(iter, ends))
            }
            b'$' => {
                iter.next();
                Some(Node::Edge(false))
            }
            b'.' => {
                iter.next();
                Some(Node::Charset(vec![b'\n'], true, None))
            }
            b'[' => {
                iter.next();
                Some(process_set(iter, ends))
            }
            b'(' => {
                iter.next();
                Some(process_group(iter, ends))
            }
            b'\\' => {
                iter.next();
                Some(process_slash(iter, true))
            }
            _ => {
                iter.next();
                Some(Node::Text(*ch, None))
            }
        };
        if let Some(ref mut n) = node {
            if let Some((min, max)) = process_repeat(iter) {
                match n {
                    Node::Text(_, r)
                    | Node::Charset(_, _, r)
                    | Node::Seq(_, r)
                    | Node::Group(_, _, r)
                    | Node::Select(_, r) => *r = Some(Repeat { min, max }),
                    _ => (),
                }
            }
        }
        if let Some(n) = node {
            seq.push(n);
        }
        dbg!(*ch as char);
        // Add the node to the parent sequence
    }
    Node::Seq(seq, None)
}

pub fn parse<'a>(re: Vec<u8>) -> Result<(), ParseError> {
    let re = pre_parse(re);
    let iter = re.iter();
    let mut ends = Vec::with_capacity(16);
    let parent = process_seq(&mut iter.peekable(), &mut ends);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer() {
        let mut num = 0;
        super::process_int(&mut &mut "1234".as_bytes().iter().peekable(), &mut num);
        assert_eq!(num, 1234);
    }

    #[test]
    fn range() {
        assert_eq!(
            super::process_repeat(&mut "?".as_bytes().iter().peekable()),
            Some((0, 1))
        );
        assert_eq!(
            super::process_repeat(&mut "*".as_bytes().iter().peekable()),
            Some((0, 65536))
        );
        assert_eq!(
            super::process_repeat(&mut "+".as_bytes().iter().peekable()),
            Some((1, 65536))
        );
    }
    #[test]
    fn range_complex_a() {
        assert_eq!(
            super::process_repeat(&mut "{8}".as_bytes().iter().peekable()),
            Some((8, 8))
        );
    }
    #[test]
    fn range_complex_b() {
        assert_eq!(
            super::process_repeat(&mut "{8,}".as_bytes().iter().peekable()),
            Some((8, 65536))
        );
    }
    #[test]
    fn range_complex_c() {
        assert_eq!(
            super::process_repeat(&mut "{,9}".as_bytes().iter().peekable()),
            Some((0, 9))
        );
    }
    #[test]
    fn slash_test_a() {
        assert_eq!(
            super::process_slash(&mut "\\".as_bytes().iter().peekable(), false),
            Node::Text(b'\\', None)
        );
    }
    #[test]
    fn slash_test_b() {
        assert_eq!(
            super::process_slash(&mut "t".as_bytes().iter().peekable(), false),
            Node::Text(b'\t', None)
        );
        assert_eq!(
            super::process_slash(&mut "r".as_bytes().iter().peekable(), false),
            Node::Text(b'\r', None)
        );
        assert_eq!(
            super::process_slash(&mut "n".as_bytes().iter().peekable(), false),
            Node::Text(b'\n', None)
        );
    }
    #[test]
    fn slash_test_c() {
        assert_eq!(
            super::process_slash(&mut r"d".as_bytes().iter().peekable(), false),
            Node::Charset((b'0'..=b'9').collect(), true, None)
        );
        assert_eq!(
            super::process_slash(&mut r"D".as_bytes().iter().peekable(), false),
            Node::Charset((b'0'..=b'9').collect(), false, None)
        );
    }
    #[test]
    fn slash_test_d() {
        assert_eq!(
            super::process_slash(&mut r"s".as_bytes().iter().peekable(), false),
            Node::Charset(vec![b'\t', b' '], true, None)
        );
        assert_eq!(
            super::process_slash(&mut r"S".as_bytes().iter().peekable(), false),
            Node::Charset(vec![b'\t', b' '], false, None)
        );
    }
    #[test]
    fn slash_test_e() {
        let mut charset = Vec::with_capacity(255);
        charset.extend_from_slice((b'A'..=b'Z').collect::<Vec<_>>().as_slice());
        charset.extend_from_slice((b'a'..=b'z').collect::<Vec<_>>().as_slice());
        charset.extend_from_slice((b'0'..=b'9').collect::<Vec<_>>().as_slice());
        charset.push(b'_');
        assert_eq!(
            super::process_slash(&mut r"w".as_bytes().iter().peekable(), false),
            Node::Charset(charset.clone(), true, None)
        );
        assert_eq!(
            super::process_slash(&mut r"W".as_bytes().iter().peekable(), false),
            Node::Charset(charset, false, None)
        );
    }
    #[test]
    fn slash_test_f() {
        assert_eq!(
            super::process_slash(&mut r"1".as_bytes().iter().peekable(), true),
            Node::Ref(1)
        );
        assert_eq!(
            super::process_slash(&mut r"2".as_bytes().iter().peekable(), true),
            Node::Ref(2)
        );
    }
    #[test]
    fn slash_test_g() {
        assert_eq!(
            super::process_slash(&mut r"a".as_bytes().iter().peekable(), true),
            Node::Text(b'a', None)
        );
    }
    #[test]
    fn set_test_a() {
        assert_eq!(
            super::process_set(&mut r"a-z".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset((b'a'..=b'z').collect::<Vec<_>>(), true, None)
        );
        assert_eq!(
            super::process_set(&mut r"A-Z".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset((b'A'..=b'Z').collect::<Vec<_>>(), true, None)
        );
        assert_eq!(
            super::process_set(&mut r"0-9".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset((b'0'..=b'9').collect::<Vec<_>>(), true, None)
        );
    }
    #[test]
    fn set_test_b() {
        assert_eq!(
            super::process_set(&mut r"e-l".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset((b'e'..=b'l').collect::<Vec<_>>(), true, None)
        );
    }
    #[test]
    fn set_test_c() {
        assert_eq!(
            super::process_set(&mut r"^e-l".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset((b'e'..=b'l').collect::<Vec<_>>(), false, None)
        );
        assert_eq!(
            super::process_set(&mut r"^0-9".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset((b'0'..=b'9').collect::<Vec<_>>(), false, None)
        );
    }
    #[test]
    fn set_test_d() {
        assert_eq!(
            super::process_set(&mut r"hello|".as_bytes().iter().peekable(), &mut Vec::new()),
            Node::Charset("hello".as_bytes().to_vec(), true, None)
        );
    }

    #[test]
    fn sub_expr_a() {
        assert_eq!(
            super::is_sub_expr(&mut r"?:".as_bytes().iter().peekable()),
            0
        );
        assert_eq!(
            super::is_sub_expr(&mut r"?a".as_bytes().iter().peekable()),
            97
        );
        assert_eq!(super::is_sub_expr(&mut r"".as_bytes().iter().peekable()), 0);
    }

    #[test]
    fn group_a() {
        let mut charset = Vec::with_capacity(255);
        charset.extend_from_slice((b'a'..=b'z').collect::<Vec<_>>().as_slice());
        charset.extend_from_slice((b'0'..=b'9').collect::<Vec<_>>().as_slice());
        assert_eq!(
            super::process_group(
                &mut r"[a-z|0-9])".as_bytes().iter().peekable(),
                &mut Vec::new()
            ),
            Node::Group(
                vec![Node::Seq(vec![Node::Charset(charset, true, None)], None)],
                91,
                None
            )
        );
    }

    #[test]
    fn group_b() {
        let mut charset = Vec::with_capacity(255);
        charset.extend_from_slice((b'a'..=b'z').collect::<Vec<_>>().as_slice());
        charset.extend_from_slice((b'0'..=b'9').collect::<Vec<_>>().as_slice());
        assert_eq!(
            super::process_group(
                &mut r"?:[a-z|0-9])".as_bytes().iter().peekable(),
                &mut Vec::new()
            ),
            Node::Group(
                vec![Node::Seq(vec![Node::Charset(charset, true, None)], None)],
                0,
                None
            )
        );
    }

    #[test]
    fn group_c() {
        let mut charset = Vec::with_capacity(255);
        charset.extend_from_slice((b'a'..=b'z').collect::<Vec<_>>().as_slice());
        charset.extend_from_slice((b'0'..=b'9').collect::<Vec<_>>().as_slice());
        assert_eq!(
            super::process_group(
                &mut r"?:https|ftp)://".as_bytes().iter().peekable(),
                &mut Vec::new()
            ),
            Node::Group(
                vec![Node::Seq(
                    vec![
                        Node::Text(104, None),
                        Node::Text(116, None),
                        Node::Text(116, None),
                        Node::Text(112, None),
                        Node::Text(115, None),
                        Node::Select(
                            vec![
                                Node::Seq(
                                    vec![
                                        Node::Text(102, None),
                                        Node::Text(116, None),
                                        Node::Text(112, None)
                                    ],
                                    None
                                ),
                                Node::Seq(
                                    vec![
                                        Node::Text(58, None),
                                        Node::Text(47, None),
                                        Node::Text(47, None)
                                    ],
                                    None
                                )
                            ],
                            None
                        )
                    ],
                    None
                )],
                0,
                None
            )
        );
    }
}
