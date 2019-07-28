use crate::error::ParseError;

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

fn process_int<'a>(iter: &mut impl Iterator<Item = &'a u8>, num: &mut i32) -> u8 {
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

fn process_defined_repeat<'a, 'b>(
    iter: &mut impl Iterator<Item = &'a u8>,
    min: &'b mut i32,
    max: &'b mut i32,
) {
    let mut p_iter = iter.peekable();
    if let Some(ch) = p_iter.peek() {
        match ch {
            b'0'..=b'9' => {
                let ch = process_int(&mut p_iter, min);
                match ch {
                    b'}' => *max = *min,
                    b',' => {
                        p_iter.next();
                        let _ = process_int(&mut p_iter, max);
                    }
                    _ => (),
                }
            }
            b',' => {
                p_iter.next();
                let _ = process_int(&mut p_iter, max);
            }
            _ => (),
        }
    }
}

fn process_repeat<'a>(iter: &mut impl Iterator<Item = &'a u8>) -> (i32, i32) {
    let mut min = 0;
    let mut max = 65536;
    while let Some(ch) = iter.next() {
        match ch {
            b'?' => max = 1,
            b'*' => (),
            b'+' => min = 1,
            b'{' => process_defined_repeat(iter, &mut min, &mut max),
            _ => break,
        }
    }
    (min, max)
}

fn process_select<'a>(
    iter: &mut impl Iterator<Item = &'a u8>,
    ends: &mut Vec<u8>) -> Node {
    ends.push(b'|');
    let mut select = Vec::new();
    while let Some(ch) = iter.next() {        
        select.push(process_seq(iter, ends));
        match ch {
            b'|' => continue,
            _ => break,
        }
    }
    ends.pop();
    Node::Select(select, None)
}

fn process_range<'a>(iter: &mut impl Iterator<Item = &'a u8>, to: &mut u8) {
    while let Some(ch) = iter.next() {
        if *ch != b']' {
            *to = *ch;
            break;
        }
    }
}

fn process_slash<'a>(iter: &mut impl Iterator<Item = &'a u8>, back_ref: bool) -> Node {
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

fn process_set<'a>(iter: &mut impl Iterator<Item = &'a u8>, _ends: &mut Vec<u8>) -> Node {
    let mut charset = Vec::with_capacity(255);
    let mut p_iter = iter.peekable();
    let mut begin = true;
    let mut prev = 0;
    let mut exclude = true;

    while let Some(ch) = p_iter.next() {
        dbg!(&ch);
        if *ch == b'^' {
            if begin {
                begin = false;
                exclude = false;
            }
            continue;
        }
        if *ch == b'-' && prev > 0 {
            let mut to = 0;
            process_range(&mut p_iter, &mut to);
            charset.append(&mut (prev..=to).collect());
            continue;
        }
        if prev > 0 {
            charset.push(prev);
        }
        if *ch == b']' {
            break;
        }
        if *ch == b'\\' {
            let node = process_slash(&mut p_iter, false);
            match node {
                Node::Charset(mut v, _, _) => charset.append(&mut v),
                _ => (),
            }
        }
        prev = *ch;
    }
    if !charset.is_empty() {
        Node::Charset(charset, exclude, None)
    } else {
        Node::Text(b'[', None)
    }
}

fn is_sub_expr<'a>(iter: &mut impl Iterator<Item = &'a u8>) -> u8 {
    let mut p_iter = iter.peekable();
    let mut is_subexp = false;
    let mut ch: u8 = 0;
    while let Some(n) = p_iter.peek() {
        match n {
            b'?' => is_subexp = true,
            b':' | b'=' | b'!' | b'>' if is_subexp => {
                ch = **n;
            }
            _ => break,
        }
        p_iter.next();
    }
    ch
}

fn process_group<'a>(
    iter: &mut impl Iterator<Item = &'a u8>,
    ends: &mut Vec<u8>,
) -> Node {
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

fn process_seq<'a>(
    iter: &mut impl Iterator<Item = &'a u8>,
    ends: &mut Vec<u8>,
) -> Node {
    let mut seq = Vec::new();
    let mut node: Option<Node> = None;
    let mut begin = true;
    while let Some(ch) = iter.next() {
        if begin {
            if *ch == b'^' {
                node = Some(Node::Edge(true));
            } else {
                begin = false;
            }
        }
        if let Some(ref mut n) = node {
            let (min, max) = process_repeat(iter);
            match n {
                Node::Text(_, r)
                | Node::Charset(_, _, r)
                | Node::Seq(_, r)
                | Node::Group(_, _, r)
                | Node::Select(_, r) => *r = Some(Repeat { min, max }),
                _ => (),
            }
        }
        // Add the node to the parent sequence
        if let Some(n) = node {
            seq.push(n);
        }
        // Check if we are at an end of a sequence or group
        if let Some(e) = ends.last() {
            if e == ch {
                break;
            }
        }
        node = match ch {
            b'|' => Some(process_select(iter, ends)),
            b'$' => Some(Node::Edge(false)),
            b'.' => Some(Node::Charset(vec![b'\n'], true, None)),
            b'[' => Some(process_set(iter, ends)),
            b'(' => Some(process_group(iter,  ends)),
            b'\\' => Some(process_slash(iter, true)),
            _ => Some(Node::Text(*ch, None)),
        };
        if let Some(n) = node {
            seq.push(n);
            node = None;
        }
    }
    Node::Seq(seq, None)
}

pub fn parse<'a>(re: Vec<u8>) -> Result<(), ParseError> {
    let re = pre_parse(re);
    let mut iter = re.iter();
    let mut ends = Vec::with_capacity(16);
    let parent = process_seq(&mut iter, &mut ends);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer() {
        let mut num = 0;
        super::process_int(&mut "1234".as_bytes().iter(), &mut num);
        assert_eq!(num, 1234);
    }

    #[test]
    fn range() {
        assert_eq!(super::process_repeat(&mut "?".as_bytes().iter()), (0, 1));
        assert_eq!(
            super::process_repeat(&mut "*".as_bytes().iter()),
            (0, 65536)
        );
        assert_eq!(
            super::process_repeat(&mut "+".as_bytes().iter()),
            (1, 65536)
        );
    }
    #[test]
    fn range_complex_a() {
        assert_eq!(super::process_repeat(&mut "{8}".as_bytes().iter()), (8, 8));
    }
    #[test]
    fn range_complex_b() {
        assert_eq!(
            super::process_repeat(&mut "{8,}".as_bytes().iter()),
            (8, 65536)
        );
    }
    #[test]
    fn range_complex_c() {
        assert_eq!(super::process_repeat(&mut "{,9}".as_bytes().iter()), (0, 9));
    }
    #[test]
    fn slash_test_a() {
        assert_eq!(
            super::process_slash(&mut "\\".as_bytes().iter(), false),
            Node::Text(b'\\', None)
        );
    }
    #[test]
    fn slash_test_b() {
        assert_eq!(
            super::process_slash(&mut "t".as_bytes().iter(), false),
            Node::Text(b'\t', None)
        );
        assert_eq!(
            super::process_slash(&mut "r".as_bytes().iter(), false),
            Node::Text(b'\r', None)
        );
        assert_eq!(
            super::process_slash(&mut "n".as_bytes().iter(), false),
            Node::Text(b'\n', None)
        );
    }
    #[test]
    fn slash_test_c() {
        assert_eq!(
            super::process_slash(&mut r"d".as_bytes().iter(), false),
            Node::Charset((b'0'..=b'9').collect(), true, None)
        );
        assert_eq!(
            super::process_slash(&mut r"D".as_bytes().iter(), false),
            Node::Charset((b'0'..=b'9').collect(), false, None)
        );
    }
    #[test]
    fn slash_test_d() {
        assert_eq!(
            super::process_slash(&mut r"s".as_bytes().iter(), false),
            Node::Charset(vec![b'\t', b' '], true, None)
        );
        assert_eq!(
            super::process_slash(&mut r"S".as_bytes().iter(), false),
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
            super::process_slash(&mut r"w".as_bytes().iter(), false),
            Node::Charset(charset.clone(), true, None)
        );
        assert_eq!(
            super::process_slash(&mut r"W".as_bytes().iter(), false),
            Node::Charset(charset, false, None)
        );
    }
    #[test]
    fn slash_test_f() {
        assert_eq!(
            super::process_slash(&mut r"1".as_bytes().iter(), true),
            Node::Ref(1)
        );
        assert_eq!(
            super::process_slash(&mut r"2".as_bytes().iter(), true),
            Node::Ref(2)
        );
    }
    #[test]
    fn slash_test_g() {
        assert_eq!(
            super::process_slash(&mut r"a".as_bytes().iter(), true),
            Node::Text(b'a', None)
        );
    }
    #[test]
    fn set_test_a() {
        assert_eq!(
            super::process_set(&mut r"a-z".as_bytes().iter(), &mut Vec::new()),
            Node::Charset((b'a'..=b'z').collect::<Vec<_>>(), true, None)
        );
        assert_eq!(
            super::process_set(&mut r"A-Z".as_bytes().iter(), &mut Vec::new()),
            Node::Charset((b'A'..=b'Z').collect::<Vec<_>>(), true, None)
        );
        assert_eq!(
            super::process_set(&mut r"0-9".as_bytes().iter(), &mut Vec::new()),
            Node::Charset((b'0'..=b'9').collect::<Vec<_>>(), true, None)
        );
    }
    #[test]
    fn set_test_b() {
        assert_eq!(
            super::process_set(&mut r"e-l".as_bytes().iter(), &mut Vec::new()),
            Node::Charset((b'e'..=b'l').collect::<Vec<_>>(), true, None)
        );
    }
    #[test]
    fn set_test_c() {
        assert_eq!(
            super::process_set(&mut r"^e-l".as_bytes().iter(), &mut Vec::new()),
            Node::Charset((b'e'..=b'l').collect::<Vec<_>>(), false, None)
        );
        assert_eq!(
            super::process_set(&mut r"^0-9".as_bytes().iter(), &mut Vec::new()),
            Node::Charset((b'0'..=b'9').collect::<Vec<_>>(), false, None)
        );
    }
    #[test]
    fn set_test_d() {
        assert_eq!(
            super::process_set(&mut r"hello|".as_bytes().iter(), &mut Vec::new()),
            Node::Charset("hello".as_bytes().to_vec(), true, None)
        );
    }
}
