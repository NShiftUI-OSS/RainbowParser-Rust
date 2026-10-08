use rainbow_parser::RAINBOW_PARSER_VERSION;
use rainbow_parser_wasm::{expand, format, parse, validate, validate_registry, version};

#[test]
fn version_matches_parser_crate() {
    assert_eq!(version(), RAINBOW_PARSER_VERSION);
}

#[test]
fn parse_returns_cli_json_envelope() {
    let json = parse(r#"Button(title: "Entrar")"#);
    assert!(json.contains(r#""ok":true"#), "{json}");
    assert!(json.contains(r#""value":"Button""#), "{json}");
}

#[test]
fn parse_error_is_json_not_a_panic() {
    let json = parse("Button(");
    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains(r#""document":null"#), "{json}");
}

#[test]
fn format_returns_canonical_source() {
    let json = format(r#"Button(title:"Entrar")"#);
    assert!(json.contains(r#""ok":true"#), "{json}");
    assert!(json.contains(r#"title: \"Entrar\""#), "{json}");
}

#[test]
fn validate_allows_template_until_concrete() {
    let source = "Button(title: #{Title})";
    let relaxed = validate(source, false);
    assert!(relaxed.contains(r#""ok":true"#), "{relaxed}");
    let concrete = validate(source, true);
    assert!(concrete.contains(r#""ok":false"#), "{concrete}");
}

#[test]
fn expand_validate_inserts_fragment() {
    let json = expand(
        "Button(title: #{Title})",
        r#"{"Title":"\"Entrar\""}"#,
        true,
    );
    assert!(json.contains(r#""ok":true"#), "{json}");
    assert!(json.contains("Entrar"), "{json}");
}

#[test]
fn expand_rejects_bad_map() {
    let json = expand("Button(title: #{Title})", "[]", false);
    assert!(json.contains(r#""ok":false"#), "{json}");
}

#[test]
fn registry_accepts_disjoint_names() {
    let json = validate_registry(
        r#"{"plugins":["Screen","Button"],"events":["Navigate","ShowToast"]}"#,
    );
    assert!(json.contains(r#""ok":true"#), "{json}");
}
