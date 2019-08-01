use crate::error::*;
use nom::bytes::complete::{tag, take};
use nom::multi::{many1, separated_list};
use nom::sequence::{delimited, tuple};
use nom::{error::ErrorKind, Err, IResult};

#[derive(Debug, PartialEq)]
enum Node {
    ExRange(Vec<u8>),
    Range(Vec<u8>),
    Alternation(Vec<Node>),
    Group(Box<Node>),
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
    let (rest, v) = delimited(tag("["), separated_list(tag("|"), multi_range), tag("]"))(input)?;
    Ok((rest, Node::Alternation(v)))
}

fn parse_group(input: &[u8]) -> IResult<&[u8], Node> {
    let (rest, v) = delimited(tag("("), alternation, tag(")"))(input)?;
    Ok((rest, Node::Group(Box::new(v))))
}

pub fn parse(re: &[u8]) {}

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
                Node::Alternation(vec![
                    Node::Range(vec![97, 98, 99]),
                    Node::Range(vec![100, 101, 102])
                ])
            ))
        );
    }

    #[test]
    fn parse_3() {
        assert_eq!(
            parse_group(b"([a-c|d-f])"),
            Ok((
                &[][..],
                Node::Group(Box::new(Node::Alternation(vec![
                    Node::Range(vec![97, 98, 99]),
                    Node::Range(vec![100, 101, 102])
                ])))
            ))
        );
    }

    #[test]
    fn parse_4() {
        assert_eq!(
            parse_group(b"([a-cd-f])"),
            Ok((
                &[][..],
                Node::Group(Box::new(Node::Alternation(vec![Node::Range(vec![
                    97, 98, 99, 100, 101, 102
                ])])))
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
}
