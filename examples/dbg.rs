fn main() {
    let src = "(2↑V)←9 8";
    let toks = apl::tokenizer::tokenize(src).unwrap();
    println!("tokens: {:?}", toks);

    // Check what parse_simple does with the inner tokens
    let inner = &toks[1..4]; // [Num(2), Prim(Take), Name("V")]
    println!("inner: {:?}", inner);

    // Use the parser's parse function
    match apl::parser::parse(inner) {
        Ok((expr, used)) => println!("parsed: {:?} (used={})", expr, used),
        Err(e) => println!("parse error: {:?}", e),
    }
}
