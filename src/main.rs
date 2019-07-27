mod error;
mod regexgen;

fn main() -> Result<(), error::ParseError> {
    let _v = regexgen::parse(b"dfsddf".to_vec())?;
    Ok(())
}
