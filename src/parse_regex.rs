use crate::error::*;
use nom::branch::alt;
use nom::bytes::complete::{tag, take};
use nom::character::complete::digit1;
use nom::combinator::{map, opt};
use nom::multi::{many1, separated_list};
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::{error::ErrorKind, Err, IResult};
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
    Group(Box<Node>, Option<Repeat<u16>>),
}

fn parse_range(input: &[u8]) -> IResult<&[u8], Node> {
    if !input.is_empty() {
        if input[0] == b'^' {
            let (rest, (s, _, e)) = tuple((take(1usize), tag("-"), take(1usize)))(&input[1..])?;
            Ok((rest, Node::ExRange((s[0]..=e[0]).collect::<Vec<_>>())))
        } else {
            let (rest, (s, _, e)) = tuple((take(1usize), tag("-"), take(1usize)))(input)?;
            Ok((rest, Node::Range((s[0]..=e[0]).collect::<Vec<_>>())))
        }
    } else {
        Err(Err::Error((input, ErrorKind::Complete)))
    }
}

fn repeater<T>(input: &[u8]) -> IResult<&[u8], Repeat<T>>
where
    T: Num,
    T: std::str::FromStr,
    T: From<T>,
    T: Copy,
    T: num_traits::cast::NumCast,
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

fn multi_range(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, v) = many1(parse_range)(input)?;
    let v = v
        .iter()
        .map(|rng| match rng {
            Node::Range(v) => v.clone(),
            _ => vec![],
        })
        .flatten()
        .collect::<Vec<_>>();
    Ok((rest, Node::Range(v)))
}

fn alternation(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, (v, repeat)) = pair(
        delimited(tag("["), separated_list(tag("|"), multi_range), tag("]")),
        opt(repeater::<u16>),
    )(input)?;
    Ok((rest, Node::Alternation(v, repeat)))
}

fn group(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, (v, repeat)) = pair(
        delimited(tag("("), alternation, tag(")")),
        opt(repeater::<u16>),
    )(input)?;
    Ok((rest, Node::Group(Box::new(v), repeat)))
}
(ch) = input.first() {
        match ch {
            b'\\' => Ok(input[1..],Node::Text(b'\\'))

        }
    }
}

pub fn parse(re: &[u8]) {
    group(re).expect("");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_1() {
        assert_eq!(
            parse_range(b"A-Z"),
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
                    Box::new(Node::Alternation(
                        vec![
                            Node::Range(vec![97, 98, 99]),
                            Node::Range(vec![100, 101, 102])
                        ],
                        None
                    )),
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
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
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
                Node::Range(vec![
                    97, 98, 99, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81,
                    82, 83, 84, 85, 86, 87, 88, 89, 90, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57
                ])
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
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
                    Some(Repeat::new(1, 65535))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f])*"),
            Ok((
                &[][..],
                Node::Group(
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
                    Some(Repeat::new(0, 65535))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f])?"),
            Ok((
                &[][..],
                Node::Group(
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
                    Some(Repeat::new(0, 1))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f]){6}"),
            Ok((
                &[][..],
                Node::Group(
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
                    Some(Repeat::new(6, 6))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f]){6,}"),
            Ok((
                &[][..],
                Node::Group(
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
                    Some(Repeat::new(6, 65535))
                )
            ))
        );
        assert_eq!(
            group(b"([a-cd-f]){6,67}"),
            Ok((
                &[][..],
                Node::Group(
                    Box::new(Node::Alternation(
                        vec![Node::Range(vec![97, 98, 99, 100, 101, 102])],
                        None
                    )),
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
}
