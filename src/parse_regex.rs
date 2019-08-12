use nom::branch::alt;
use nom::bytes::complete::{is_a, tag, take};
use nom::character::complete::{alpha1, digit1};
use nom::combinator::{map, opt};
use nom::multi::{many1, separated_list};
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::{error::ErrorKind, IResult};
use num_traits::{cast, Num};
use std::str::from_utf8_unchecked;

#[derive(Debug, PartialEq)]
struct Repeat<T: Num> {
    min: T,
    max: T,
}

impl<T> Repeat<T>
where
    T: Num,
{
    fn new(min: T, max: T) -> Self {
        Self { min, max }
    }
}

#[derive(Debug, PartialEq)]
enum Node {
    ExRange(Vec<u8>),
    Range(Vec<u8>),
    Alternation(Vec<Node>, Option<Repeat<u16>>),
    Group(Vec<Node>, Option<Repeat<u16>>),
    Text(Vec<u8>),
    Ref(u8),
}

fn text(input: &[u8]) -> IResult<&[u8], Node> {
    const ALPHANUM: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    map(is_a(ALPHANUM), |s: &[u8]| Node::Text(s.to_vec()))(input)
}

fn range(input: &[u8]) -> IResult<&[u8], Node> {
    alt((
        map(
            preceded(tag("^"), tuple((take(1usize), tag("-"), take(1usize)))),
            |(s, _, e): (&[u8], &[u8], &[u8])| Node::ExRange((s[0]..=e[0]).collect::<Vec<_>>()),
        ),
        map(
            tuple((take(1usize), tag("-"), take(1usize))),
            |(s, _, e): (&[u8], &[u8], &[u8])| Node::Range((s[0]..=e[0]).collect::<Vec<_>>()),
        ),
        backslash,
        text,
    ))(input)
}

fn repeater<T>(input: &[u8]) -> IResult<&[u8], Repeat<T>>
where
    T: Num + std::str::FromStr + From<T> + Copy + num_traits::cast::NumCast,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    let to_type = |v: &[u8]| {
        let str_num = unsafe { from_utf8_unchecked(v) };
        str_num.parse::<T>().unwrap()
    };

    alt((
        map(tag("*"), |_| {
            Repeat::new(cast(0).unwrap(), cast(65535).unwrap())
        }),
        map(tag("?"), |_| {
            Repeat::new(cast(0).unwrap(), cast(1).unwrap())
        }),
        map(tag("+"), |_| {
            Repeat::new(cast(1).unwrap(), cast(65535).unwrap())
        }),
        map(delimited(tag("{"), digit1, tag("}")), move |v| {
            let v = to_type(v);
            Repeat::new(v, v)
        }),
        map(
            delimited(tag("{"), terminated(digit1, tag(",")), tag("}")),
            move |v| Repeat::new(to_type(v), cast(65535).unwrap()),
        ),
        map(
            delimited(tag("{"), preceded(tag(","), digit1), tag("}")),
            move |v| Repeat::new(cast(0).unwrap(), to_type(v)),
        ),
        map(
            delimited(tag("{"), tuple((digit1, tag(","), digit1)), tag("}")),
            move |(s, _, e)| Repeat::new(to_type(s), to_type(e)),
        ),
    ))(input)
}

fn multi_range(input: &[u8]) -> IResult<&[u8], Vec<Node>> {
    let (rest, v) = many1(range)(input)?;
    Ok((rest, v))
}

fn alternation(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, (v, repeat)) = pair(
        delimited(tag("["), separated_list(tag("|"), multi_range), tag("]")),
        opt(repeater::<u16>),
    )(input)?;
    let v = v.into_iter().flatten().map(|v| v).collect();
    Ok((rest, Node::Alternation(v, repeat)))
}

fn set(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, v) = separated_list(tag("|"), multi_range)(input)?;
    let v = v.into_iter().flatten().collect();
    Ok((rest, Node::Alternation(v, None)))
}

fn backslash(input: &[u8]) -> IResult<&[u8], Node> {
    alt((
        map(tag("\\w"), |_| {
            let ext = (b'A'..=b'Z')
                .chain(b'a'..=b'z')
                .chain(b'0'..=b'9')
                .collect();
            Node::Range(ext)
        }),
        map(tag("\\d"), |_| {
            let ext = (b'0'..=b'9').collect();
            Node::Range(ext)
        }),
        map(tag("\\s"), |_| Node::Range(vec![b'\t', b'\r', b'\n', b' '])),
        map(tag("\\W"), |_| {
            let ext = (b'A'..=b'Z')
                .chain(b'a'..=b'z')
                .chain(b'0'..=b'9')
                .collect();
            Node::ExRange(ext)
        }),
        map(tag("\\D"), |_| {
            let ext = (b'0'..=b'9').collect();
            Node::ExRange(ext)
        }),
        map(tag("\\S"), |_| {
            Node::ExRange(vec![b'\t', b'\r', b'\n', b' '])
        }),
        map(tag("\\|"), |_| Node::Range(vec![b'|'])),
        map(terminated(tag("\\"), digit1), |dig: &[u8]| {
            Node::Ref(std::str::from_utf8(dig).unwrap().parse::<u8>().unwrap())
        }),
        map(terminated(tag("\\"), alpha1), |v: &[u8]| {
            Node::Range(v.to_vec())
        }),
        map(tag("\\"), |_| Node::Range(vec![b'\\'])),
    ))(input)
}

fn group(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, (v, repeat)) = pair(
        delimited(tag("("), sequence, tag(")")),
        opt(repeater::<u16>),
    )(input)?;
    Ok((rest, Node::Group(v, repeat)))
}

fn sequence(input: &[u8]) -> IResult<&[u8], Vec<Node>> {
    many1(alt((text, group, alternation, backslash)))(input)
}

pub fn parse(re: &[u8]) {
    sequence(re).expect("");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_1() {
        assert_eq!(
            range(b"A-Z"),
            Ok((&[][..], Node::Range((b'A'..=b'Z').collect::<Vec<_>>())))
        );
    }
    #[test]
    fn parse_2() {
        assert_eq!(
            alternation(b"[a-c|d-f]"),
            Ok((
                &[][..],
                Node::Alternation(
                    vec![
                        Node::Range(vec![97, 98, 99]),
                        Node::Range(vec![100, 101, 102])
                    ],
                    None
                )
            ))
        );
    }

    #[test]
    fn parse_3() {
        assert_eq!(
            group(b"([a-c|d-f])"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    None
                )
            ))
        );
    }

    #[test]
    fn parse_4() {
        assert_eq!(
            group(b"([a-cd-f])"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    None
                )
            ))
        );
    }

    #[test]
    fn parse_5() {
        assert_eq!(
            multi_range(b"a-cA-Z0-9"),
            Ok((
                &[][..],
                vec![
                    Node::Range(vec![97, 98, 99]),
                    Node::Range(vec![
                        65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83,
                        84, 85, 86, 87, 88, 89, 90
                    ]),
                    Node::Range(vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57])
                ]
            ))
        );
    }

    #[test]
    fn parse_6() {
        assert_eq!(repeater::<u16>(b"+"), Ok((&[][..], Repeat::new(1, 65535))));
        assert_eq!(repeater::<u16>(b"*"), Ok((&[][..], Repeat::new(0, 65535))));
        assert_eq!(repeater::<u16>(b"?"), Ok((&[][..], Repeat::new(0, 1))));
        assert_eq!(repeater::<u16>(b"{8}"), Ok((&[][..], Repeat::new(8, 8))));
        assert_eq!(
            repeater::<u16>(b"{8,}"),
            Ok((&[][..], Repeat::new(8, 65535)))
        );
        assert_eq!(
            repeater::<u16>(b"{,7892}"),
            Ok((&[][..], Repeat::new(0, 7892)))
        );
        assert_eq!(
            repeater::<u16>(b"{456,7892}"),
            Ok((&[][..], Repeat::new(456, 7892)))
        );
    }

    #[test]
    fn parse_7() {
        assert_eq!(
            group(b"([a-cd-f])+"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    Some(Repeat::new(1, 65535))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f])*"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    Some(Repeat::new(0, 65535))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f])?"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    Some(Repeat::new(0, 1))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f]){6}"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    Some(Repeat::new(6, 6))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f]){6,}"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    Some(Repeat::new(6, 65535))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f]){6,67}"),
            Ok((
                &[][..],
                Node::Group(
                    vec![Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )],
                    Some(Repeat::new(6, 67))
                )
            ))
        );
    }
    #[test]
    fn parse_8() {
        assert_eq!(
            alternation(b"[a-c|d-f]+"),
            Ok((
                &[][..],
                Node::Alternation(
                    vec![
                        Node::Range(vec![97, 98, 99]),
                        Node::Range(vec![100, 101, 102])
                    ],
                    Some(Repeat::new(1, 65535))
                )
            ))
        );
        assert_eq!(
            alternation(b"[a-c|d-f]{89,}"),
            Ok((
                &[][..],
                Node::Alternation(
                    vec![
                        Node::Range(vec![97, 98, 99]),
                        Node::Range(vec![100, 101, 102])
                    ],
                    Some(Repeat::new(89, 65535))
                )
            ))
        );
        assert_eq!(
            alternation(b"[a-c]{89,}"),
            Ok((
                &[][..],
                Node::Alternation(
                    vec![Node::Range(vec![97, 98, 99]),],
                    Some(Repeat::new(89, 65535))
                )
            ))
        );
    }

    #[test]
    fn parse_9() {
        use nom::bytes::complete::take_while1;
        assert_eq!(
            take_while1::<_, &[u8], (&[u8], ErrorKind)>(|c| !(c == b'^' || c == b'\\'))(
                b"dabc\\def"
            ),
            Ok((&b"\\def"[..], &b"dabc"[..]))
        );
        assert_eq!(
            take_while1::<_, &[u8], (&[u8], ErrorKind)>(|c| !(c == b'^' || c == b'\\'))(
                b"dabc^def"
            ),
            Ok((&b"^def"[..], &b"dabc"[..]))
        );
    }
    #[test]
    fn parse_10() {
        assert_eq!(
            range(b"foobar"),
            Ok((&[][..], Node::Text(b"foobar".to_vec())))
        );
        assert_eq!(
            alternation(b"[foobar|https]"),
            Ok((
                &[][..],
                Node::Alternation(
                    vec![
                        Node::Text(b"foobar".to_vec()),
                        Node::Text(b"https".to_vec())
                    ],
                    None
                )
            ))
        );
    }
    #[test]
    fn parse_11() {
        let res: Vec<u8> = (b'A'..=b'Z')
            .chain(b'a'..=b'z')
            .chain(b'0'..=b'9')
            .collect();
        assert_eq!(
            sequence(b"\\wAbcdef"),
            Ok((
                &[][..],
                vec![Node::Range(res), Node::Text(b"Abcdef".to_vec())]
            ))
        );
    }

    #[test]
    fn parse_12() {
        assert_eq!(
            set(b"a-c|g-k"),
            Ok((
                &[][..],
                Node::Alternation(
                    vec![
                        Node::Range(vec![97, 98, 99]),
                        Node::Range(vec![103, 104, 105, 106, 107])
                    ],
                    None
                )
            ))
        );
    }

    #[test]
    fn parse_13() {
        println!("{:?}", super::parse(b"([a-z|A-Z])"));
    }
}
/*
- 07900492482
====
2400
3600
====
Mobeen
====
*/
