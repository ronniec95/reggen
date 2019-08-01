mod error;
//mod regexgen;
mod parse_regex;

//use crate::regldg::*;

fn main() -> Result<(), error::ParseError> {
    //let _v = regexgen::parse(b"dfsddf".to_vec())?;
    let regex = b"(a-z|A-Z|:;+)[cd]{2}\\1";
    let _v = parse_regex::parse(&regex.to_vec());
    Ok(())
}
