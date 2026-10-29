use super::*;

fn kinds(text: &str) -> Vec<(&str, TokenKind)> {
    tokenize(text)
        .into_iter()
        .filter(|(_, kind)| *kind != TokenKind::Whitespace)
        .map(|(range, kind)| (&text[range], kind))
        .collect()
}

#[test]
fn covers_the_whole_text() {
    let text = "x = f'a{b}' # hé\n\"\"\"doc\nmore\"\"\" 1.5e-3";
    let tokens = tokenize(text);
    assert_eq!(tokens.first().unwrap().0.start, 0);
    assert_eq!(tokens.last().unwrap().0.end, text.len());
    for pair in tokens.windows(2) {
        assert_eq!(pair[0].0.end, pair[1].0.start);
    }
}

#[test]
fn recognizes_python_constructs() {
    use TokenKind::*;
    assert_eq!(
        kinds("def box(self, size=1.5):  # comment"),
        [
            ("def", Keyword),
            ("box", FunctionName),
            ("(", Bracket),
            ("self", SelfParam),
            (",", Operator),
            ("size", Identifier),
            ("=", Operator),
            ("1.5", Number),
            (")", Bracket),
            (":", Operator),
            ("# comment", Comment),
        ]
    );
    assert_eq!(kinds("class Part:")[1], ("Part", ClassName));
    assert_eq!(
        kinds("@cq.Workplane.method")[0],
        ("@cq.Workplane.method", Decorator)
    );
    assert_eq!(kinds("show_object(result)")[0], ("show_object", EditorApi));
    assert_eq!(kinds("x.print()")[2], ("print", Identifier));
    assert_eq!(kinds("None")[0], ("None", Constant));
}

#[test]
fn handles_strings() {
    use TokenKind::*;
    assert_eq!(
        kinds(r#"rb'\d' "a\"b""#),
        [(r"rb'\d'", String), (r#""a\"b""#, String)]
    );
    assert_eq!(kinds("'''one\ntwo''' x")[0].1, String);
    assert_eq!(kinds("'open\nx")[0], ("'open", String));
    // An identifier ending in a prefix letter is not a string prefix.
    assert_eq!(kinds("bar'x'")[0], ("bar", Identifier));
}

#[test]
fn handles_numbers() {
    use TokenKind::*;
    assert_eq!(
        kinds("0x1F 1_000 2.5e+10 .5 3j"),
        [
            ("0x1F", Number),
            ("1_000", Number),
            ("2.5e+10", Number),
            (".5", Number),
            ("3j", Number),
        ]
    );
}
