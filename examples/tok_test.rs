fn main() {
    for src in ["(2 2⍴1 2 3 4)(,⍤0 1)2 2⍴5 6 7 8", "1 2 3(,⍤0 1)4 5 6"] {
        match apl::tokenizer::tokenize(src) {
            Ok(t) => println!("{src:?} -> {t:?}"),
            Err(e) => println!("{src:?} -> ERR {e:?}"),
        }
    }
}
