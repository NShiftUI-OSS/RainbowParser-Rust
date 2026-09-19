use std::collections::BTreeMap;

use rainbow_parser::{
    collect_embedded_regions, decode, encode, expand_and_validate, expand_placeholders,
    format_embedded, format_source, lex, parse_substitution_map, parse_success_to_json,
    validate_concrete_source, validate_source, EmbeddedLanguage, RainbowBlock, RainbowDocument,
    RainbowFormatStyle, RainbowName, RainbowNode, RainbowObjectEntry, RainbowParameter,
    RainbowTaggedBody, RainbowTokenKind, RainbowUseDeclaration, RainbowValue,
    RAINBOW_PARSER_VERSION,
};

#[test]
fn lexer_scans_punctuation_and_eof() {
    let tokens = lex("(){}[]:,@").expect("lexing should succeed");

    assert_eq!(
        tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            RainbowTokenKind::LeftParen,
            RainbowTokenKind::RightParen,
            RainbowTokenKind::LeftBrace,
            RainbowTokenKind::RightBrace,
            RainbowTokenKind::LeftBracket,
            RainbowTokenKind::RightBracket,
            RainbowTokenKind::Colon,
            RainbowTokenKind::Comma,
            RainbowTokenKind::At,
            RainbowTokenKind::Eof,
        ]
    );
}

#[test]
fn lexer_scans_semver_as_version_token() {
    let tokens = lex("1.0.0 1.25").expect("lexing should succeed");

    assert_eq!(
        tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            RainbowTokenKind::Version("1.0.0".to_string()),
            RainbowTokenKind::Double(1.25),
            RainbowTokenKind::Eof,
        ]
    );
}

#[test]
fn parser_decodes_use_declarations_and_nodes() {
    let document = decode(
        r#"
        use Screen@1.0.0
        use Button@2.1.0
        use ShowToast@1.3.0

        Screen(name: "Home") {
          Button(title: "Hi") {
            OnTap {
              ShowToast(message: "ok")
            }
          }
        }
        "#,
    )
    .expect("parse should succeed");

    assert_eq!(
        document.uses,
        vec![
            RainbowUseDeclaration::new("Screen", "1.0.0"),
            RainbowUseDeclaration::new("Button", "2.1.0"),
            RainbowUseDeclaration::new("ShowToast", "1.3.0"),
        ]
    );
    assert_eq!(document.nodes.len(), 1);
    assert_eq!(document.nodes[0].name, "Screen");
}

#[test]
fn printer_encodes_use_declarations() {
    let document = RainbowDocument::with_uses(
        vec![
            RainbowUseDeclaration::new("Screen", "1.0.0"),
            RainbowUseDeclaration::new("Button", "2.0.0"),
        ],
        vec![RainbowNode::with_parameters(
            "Screen",
            vec![RainbowParameter::new(
                "name",
                RainbowValue::String("Home".to_string()),
            )],
        )],
    );

    assert_eq!(
        encode(&document),
        "use Screen@1.0.0\nuse Button@2.0.0\n\nScreen(name: \"Home\")"
    );
}

#[test]
fn lexer_scans_whitespace_only_input_as_eof() {
    let tokens = lex(" \r\n\t").expect("lexing should succeed");

    assert_eq!(tokens[0].kind, RainbowTokenKind::Eof);
    assert_eq!(tokens[0].range.start.line, 2);
    assert_eq!(tokens[0].range.start.column, 2);
}

#[test]
fn lexer_scans_literals_and_keywords() {
    let tokens = lex(r#"Button title "hello\nworld" -12 3.5 true false null .primary"#)
        .expect("lexing should succeed");

    assert_eq!(
        tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            RainbowTokenKind::Identifier("Button".to_string()),
            RainbowTokenKind::Identifier("title".to_string()),
            RainbowTokenKind::String("hello\nworld".to_string()),
            RainbowTokenKind::Int(-12),
            RainbowTokenKind::Double(3.5),
            RainbowTokenKind::Bool(true),
            RainbowTokenKind::Bool(false),
            RainbowTokenKind::Null,
            RainbowTokenKind::DotIdentifier("primary".to_string()),
            RainbowTokenKind::Eof,
        ]
    );
}

#[test]
fn lexer_reports_invalid_escape() {
    let error = lex(r#""bad \x escape""#).expect_err("invalid escape should fail");

    assert_eq!(error.diagnostics[0].code, "rainbow.lexer.invalidEscape");
    assert_eq!(
        error.diagnostics[0].message,
        r#"Invalid escape sequence \x."#
    );
}

#[test]
fn lexer_reports_integer_overflow() {
    let error =
        lex("999999999999999999999999999999999999").expect_err("integer overflow should fail");

    assert_eq!(error.diagnostics[0].code, "rainbow.lexer.invalidNumber");
}

#[test]
fn parser_decodes_nested_rainbow_document() {
    let document = decode(
        r#"
        Screen(name: "Home") {
          Button(title: "Entrar", variant: .primary) {
            OnTap {
              Navigate(to: "Dashboard")
            }
          }
        }
        "#,
    )
    .expect("parse should succeed");

    assert_eq!(
        document,
        RainbowDocument::new(vec![RainbowNode::with_children(
            "Screen",
            vec![RainbowParameter::new(
                "name",
                RainbowValue::String("Home".to_string())
            )],
            vec![RainbowNode::with_children(
                "Button",
                vec![
                    RainbowParameter::new("title", RainbowValue::String("Entrar".to_string())),
                    RainbowParameter::new(
                        "variant",
                        RainbowValue::Identifier("primary".to_string())
                    ),
                ],
                vec![RainbowNode::with_children(
                    "OnTap",
                    vec![],
                    vec![RainbowNode::with_parameters(
                        "Navigate",
                        vec![RainbowParameter::new(
                            "to",
                            RainbowValue::String("Dashboard".to_string())
                        )]
                    )]
                )]
            )]
        )])
    );
}

#[test]
fn parser_decodes_all_supported_value_shapes() {
    let document = decode(
        r#"
        Node(
          text: "value",
          int: -1,
          double: 1.25,
          enabled: true,
          disabled: false,
          variant: .primary,
          items: ["a", 1, null],
          meta: (id: "home", "dash-key": false),
          emptyArray: [],
          emptyObject: ()
        )
        "#,
    )
    .expect("parse should succeed");

    assert_eq!(
        document.nodes[0].parameters,
        vec![
            RainbowParameter::new("text", RainbowValue::String("value".to_string())),
            RainbowParameter::new("int", RainbowValue::Int(-1)),
            RainbowParameter::new("double", RainbowValue::Double(1.25)),
            RainbowParameter::new("enabled", RainbowValue::Bool(true)),
            RainbowParameter::new("disabled", RainbowValue::Bool(false)),
            RainbowParameter::new("variant", RainbowValue::Identifier("primary".to_string())),
            RainbowParameter::new(
                "items",
                RainbowValue::Array(vec![
                    RainbowValue::String("a".to_string()),
                    RainbowValue::Int(1),
                    RainbowValue::Null,
                ])
            ),
            RainbowParameter::new(
                "meta",
                RainbowValue::Object(vec![
                    RainbowObjectEntry::new("id", RainbowValue::String("home".to_string())),
                    RainbowObjectEntry::new("dash-key", RainbowValue::Bool(false)),
                ])
            ),
            RainbowParameter::new("emptyArray", RainbowValue::Array(vec![])),
            RainbowParameter::new("emptyObject", RainbowValue::Object(vec![])),
        ]
    );
}

#[test]
fn parser_preserves_empty_arguments_and_blocks() {
    let document = decode(
        r#"
        Root() {
          EmptyBlock {}
          Leaf
        }
        "#,
    )
    .expect("parse should succeed");

    let root = &document.nodes[0];
    assert!(root.parameters.is_empty());
    assert!(root.has_block());
    assert!(root.children()[0].has_block());
    assert!(!root.children()[1].has_block());
}

#[test]
fn parser_allows_trailing_commas_in_arrays() {
    let document = decode(
        r#"
        Node(
          values: [.one, .two,],
          object: (first: 1, second: 2)
        )
        "#,
    )
    .expect("parse should succeed");

    assert_eq!(
        document.nodes[0].parameters,
        vec![
            RainbowParameter::new(
                "values",
                RainbowValue::Array(vec![
                    RainbowValue::Identifier("one".to_string()),
                    RainbowValue::Identifier("two".to_string()),
                ])
            ),
            RainbowParameter::new(
                "object",
                RainbowValue::Object(vec![
                    RainbowObjectEntry::new("first", RainbowValue::Int(1)),
                    RainbowObjectEntry::new("second", RainbowValue::Int(2)),
                ])
            ),
        ]
    );
}

#[test]
fn parser_rejects_trailing_comma_after_last_object_entry() {
    let error = decode(r#"Node(object: (first: 1, second: 2,))"#)
        .expect_err("trailing object comma should fail");

    assert_eq!(error.diagnostics[0].code, "rainbow.parser.unexpectedToken");
    assert_eq!(
        error.diagnostics[0].message,
        "Trailing comma is not allowed after the last object entry."
    );
}

#[test]
fn parser_rejects_trailing_comma_after_last_parameter() {
    let error =
        decode(r#"Button(title: "Entrar",)"#).expect_err("trailing parameter comma should fail");

    assert_eq!(error.diagnostics[0].code, "rainbow.parser.unexpectedToken");
    assert_eq!(
        error.diagnostics[0].message,
        "Trailing comma is not allowed after the last parameter."
    );
}

#[test]
fn parser_rejects_bare_enum_identifier_without_dot() {
    let error = decode("Button(variant: primary)").expect_err("bare enum should fail");

    assert_eq!(error.diagnostics[0].code, "rainbow.parser.unexpectedToken");
    assert_eq!(
        error.diagnostics[0].message,
        "Enum identifier values must start with '.'; did you mean '.primary'?"
    );
}

#[test]
fn parser_reports_missing_value() {
    let error = decode("Button(title: )").expect_err("missing value should fail");

    assert_eq!(error.diagnostics[0].message, "Expected value.");
}

#[test]
fn parser_reports_missing_object_key_and_closing_paren() {
    let error = decode("Node(meta: ( : true)").expect_err("invalid object should fail");
    let messages = error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(messages.contains(&"Expected object key."));
    assert!(
        messages.iter().any(|message| {
            *message == "Expected ')' after object."
                || *message == "Expected ')' after parameter list."
        }),
        "unexpected messages: {messages:?}"
    );
}

#[test]
fn printer_encodes_canonical_rainbow() {
    let document = RainbowDocument::new(vec![RainbowNode::with_children(
        "Screen",
        vec![RainbowParameter::new(
            "name",
            RainbowValue::String("Home".to_string()),
        )],
        vec![RainbowNode::with_children(
            "Button",
            vec![
                RainbowParameter::new("title", RainbowValue::String("Entrar".to_string())),
                RainbowParameter::new("variant", RainbowValue::Identifier("primary".to_string())),
            ],
            vec![RainbowNode {
                name: RainbowName::ident("OnTap"),
                parameters: vec![],
                block: Some(RainbowBlock::new(vec![])),
                leading: Default::default(),
            }],
        )],
    )]);

    assert_eq!(
        encode(&document),
        r#"Screen(name: "Home") {
  Button(
    title: "Entrar",
    variant: .primary
  ) {
    OnTap {
    }
  }
}"#
    );
}

#[test]
fn printer_breaks_parameters_when_two_or_more() {
    let document = RainbowDocument::new(vec![RainbowNode::with_children(
        "Screen",
        vec![
            RainbowParameter::new("title", RainbowValue::String("Exemplo".to_string())),
            RainbowParameter::new("subtitle", RainbowValue::String("Identação".to_string())),
            RainbowParameter::new("enabled", RainbowValue::Bool(true)),
        ],
        vec![RainbowNode::with_parameters(
            "Text",
            vec![RainbowParameter::new(
                "value",
                RainbowValue::String("Hi".to_string()),
            )],
        )],
    )]);

    assert_eq!(
        encode(&document),
        r#"Screen(
  title: "Exemplo",
  subtitle: "Identação",
  enabled: true
) {
  Text(value: "Hi")
}"#
    );
}

#[test]
fn printer_encodes_all_value_shapes() {
    let document = RainbowDocument::new(vec![RainbowNode::with_parameters(
        "Node",
        vec![
            RainbowParameter::new(
                "text",
                RainbowValue::String("quote: \" slash: \\ newline: \n".to_string()),
            ),
            RainbowParameter::new(
                "control",
                RainbowValue::String("tab: \t return: \r null: \0".to_string()),
            ),
            RainbowParameter::new("int", RainbowValue::Int(-2)),
            RainbowParameter::new("double", RainbowValue::Double(2.5)),
            RainbowParameter::new("enabled", RainbowValue::Bool(true)),
            RainbowParameter::new("disabled", RainbowValue::Bool(false)),
            RainbowParameter::new("variant", RainbowValue::Identifier("primary".to_string())),
            RainbowParameter::new(
                "items",
                RainbowValue::Array(vec![
                    RainbowValue::String("a".to_string()),
                    RainbowValue::Int(1),
                    RainbowValue::Null,
                ]),
            ),
            RainbowParameter::new(
                "meta",
                RainbowValue::Object(vec![
                    RainbowObjectEntry::new("id", RainbowValue::String("home".to_string())),
                    RainbowObjectEntry::new("dash-key", RainbowValue::Bool(false)),
                    RainbowObjectEntry::new("", RainbowValue::Int(0)),
                    RainbowObjectEntry::new("1bad", RainbowValue::Int(1)),
                    RainbowObjectEntry::new("a-", RainbowValue::Int(2)),
                    RainbowObjectEntry::new("_id", RainbowValue::Int(4)),
                    RainbowObjectEntry::new("A1", RainbowValue::Int(5)),
                ]),
            ),
        ],
    )]);

    assert_eq!(
        encode(&document),
        r#"Node(
  text: "quote: \" slash: \\ newline: \n",
  control: "tab: \t return: \r null: \0",
  int: -2,
  double: 2.5,
  enabled: true,
  disabled: false,
  variant: .primary,
  items: ["a", 1, null],
  meta: (id: "home", "dash-key": false, "": 0, "1bad": 1, "a-": 2, _id: 4, A1: 5)
)"#
    );
}

#[test]
fn printer_uses_custom_format_style() {
    let document = RainbowDocument::new(vec![RainbowNode::with_children(
        "Root",
        vec![],
        vec![RainbowNode::new("Child")],
    )]);
    let style = RainbowFormatStyle::new("    ", "\r\n");

    assert_eq!(
        rainbow_parser::format_document(&document, &style),
        "Root {\r\n    Child\r\n}"
    );
}

#[test]
fn canonical_round_trip_preserves_structure() {
    let source = r#"Button( title:"Entrar", meta:(id:"login") ){OnTap{Navigate(to:"Home")}}"#;
    let first_document = decode(source).expect("first parse should succeed");
    let encoded = encode(&first_document);
    let second_document = decode(&encoded).expect("second parse should succeed");

    assert_eq!(second_document, first_document);
    assert_eq!(
        encoded,
        r#"Button(
  title: "Entrar",
  meta: (id: "login")
) {
  OnTap {
    Navigate(to: "Home")
  }
}"#
    );
}

#[test]
fn parser_exposes_version_and_json_response() {
    let document = decode(r#"Button(title: "Entrar")"#).expect("parse should succeed");
    let json = parse_success_to_json(&document);

    assert_eq!(RAINBOW_PARSER_VERSION, "0.1.0-beta.1");
    assert!(json.contains(r#""ok":true"#));
    assert!(json.contains(r#""kind":"ident""#));
    assert!(json.contains(r#""value":"Button""#));
    assert!(json.contains(r#""kind":"string""#));
}

#[test]
fn name_registry_accepts_disjoint_plugin_and_event_names() {
    let registry = rainbow_parser::decode_name_registry(
        r#"{
          "plugins": ["Screen", "Button"],
          "events": ["Navigate", "ShowToast"]
        }"#,
    )
    .expect("disjoint registry should succeed");

    assert_eq!(
        registry.plugins.iter().cloned().collect::<Vec<_>>(),
        vec!["Button".to_string(), "Screen".to_string()]
    );
    assert_eq!(
        registry.events.iter().cloned().collect::<Vec<_>>(),
        vec!["Navigate".to_string(), "ShowToast".to_string()]
    );
}

#[test]
fn name_registry_rejects_shared_plugin_and_event_names() {
    let error = rainbow_parser::decode_name_registry(
        r#"{
          "plugins": ["Screen", "Navigate"],
          "events": ["Navigate", "ShowToast"]
        }"#,
    )
    .expect_err("overlapping names should fail");

    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(
        error.diagnostics[0].code,
        "rainbow.semantic.nameKindConflict"
    );
    assert_eq!(
        error.diagnostics[0].message,
        "Name \"Navigate\" cannot be both a plugin and an event; choose one kind."
    );
}

#[test]
fn validate_source_allows_distinct_plugin_and_event_positions() {
    let document = rainbow_parser::validate_source(
        r#"
        Screen {
          Button {
            OnTap {
              Navigate(to: "Home")
            }
          }
        }
        "#,
    )
    .expect("distinct positions should succeed");

    assert_eq!(document.nodes[0].name, "Screen");
}

#[test]
fn validate_source_rejects_same_name_in_plugin_and_event_positions() {
    let error = rainbow_parser::validate_source(
        r#"
        Foo {
          OnTap {
            Foo(to: "Home")
          }
        }
        "#,
    )
    .expect_err("same name in both positions should fail");

    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(
        error.diagnostics[0].code,
        "rainbow.semantic.nameKindConflict"
    );
    assert!(error.diagnostics[0].message.contains("\"Foo\""));
    assert!(error.diagnostics[0].message.contains("plugin"));
    assert!(error.diagnostics[0].message.contains("event"));
}

#[test]
fn is_trigger_node_name_matches_on_pascal_case() {
    assert!(rainbow_parser::is_trigger_node_name("OnTap"));
    assert!(rainbow_parser::is_trigger_node_name("OnSuccess"));
    assert!(!rainbow_parser::is_trigger_node_name("Navigate"));
    assert!(!rainbow_parser::is_trigger_node_name("onTap"));
    assert!(!rainbow_parser::is_trigger_node_name("On"));
}

#[test]
fn is_upper_camel_case_name_matches_macro_rules() {
    assert!(rainbow_parser::is_upper_camel_case_name("Screen"));
    assert!(rainbow_parser::is_upper_camel_case_name("OnTap"));
    assert!(rainbow_parser::is_upper_camel_case_name("SendHTTPRequest"));
    assert!(!rainbow_parser::is_upper_camel_case_name("dasdadsa"));
    assert!(!rainbow_parser::is_upper_camel_case_name("onTap"));
    assert!(!rainbow_parser::is_upper_camel_case_name("Button2"));
    assert!(!rainbow_parser::is_upper_camel_case_name("_Screen"));
}

#[test]
fn validate_source_rejects_lowercase_plugin_name() {
    let error = rainbow_parser::validate_source(
        r#"
        Screen {
          dasdadsa
        }
        "#,
    )
    .expect_err("lowercase plugin name should fail");

    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(
        error.diagnostics[0].code,
        "rainbow.semantic.invalidNodeName"
    );
    assert!(error.diagnostics[0].message.contains("\"dasdadsa\""));
    assert!(error.diagnostics[0].message.contains("plugin"));
    assert!(error.diagnostics[0].message.contains("UpperCamelCase"));
}

#[test]
fn validate_source_rejects_lowercase_use_pin_name() {
    let error = rainbow_parser::validate_source("use screen@1.0.0\nScreen {}")
        .expect_err("lowercase use pin should fail");

    assert!(error.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "rainbow.semantic.invalidNodeName"
        && diagnostic.message.contains("use pin")
        && diagnostic.message.contains("\"screen\"")));
}

#[test]
fn lexer_scans_placeholders() {
    let tokens = lex("#{PlaceholderPluginName}").expect("lexing should succeed");
    assert!(matches!(
        &tokens[0].kind,
        RainbowTokenKind::Placeholder(name) if name == "PlaceholderPluginName"
    ));
}

#[test]
fn parser_decodes_template_use_and_placeholders() {
    let document = decode(
        r#"
        use Screen@1.0.0
        use #{UsePin}
        Screen {
          #{PlaceholderPluginName}
          Button(state: #{PlaceholderParamName})
        }
        "#,
    )
    .expect("template parse should succeed");

    assert_eq!(
        document.uses[1],
        RainbowUseDeclaration::template_name("UsePin")
    );
    assert_eq!(
        document.nodes[0].children()[0].name,
        RainbowName::placeholder("PlaceholderPluginName")
    );
    assert_eq!(
        document.nodes[0].children()[1].parameters[0].value,
        RainbowValue::Placeholder("PlaceholderParamName".to_string())
    );
}

#[test]
fn expand_placeholders_replaces_every_occurrence() {
    let source = std::fs::read_to_string("fixtures/templates/example.rbw").unwrap_or_else(|_| {
        r#"
use Screen@1.0.0
use #{UsePin}
Screen {
  #{PlaceholderPluginName}
  Button(state: #{PlaceholderParamName})
}
"#
        .to_string()
    });
    let map = parse_substitution_map(
        r#"{"UsePin":"PromoBanner@1.0.0","PlaceholderPluginName":"PromoBanner","PlaceholderParamName":"\"loading\""}"#,
    )
    .expect("map should parse");

    let expanded = expand_and_validate(&source, &map).expect("expand should succeed");
    assert!(
        expanded.contains("use PromoBanner@1.0.0\n")
            || expanded.contains("use PromoBanner@1.0.0\r\n")
    );
    assert!(expanded.contains("PromoBanner"));
    assert!(expanded.contains("state: \"loading\""));
    assert!(!expanded.contains("#{"));
}

#[test]
fn expand_placeholders_reports_missing_keys() {
    let error = expand_placeholders("Button(state: #{Missing})", &BTreeMap::new())
        .expect_err("missing key should fail");

    assert_eq!(
        error.diagnostics[0].code,
        "rainbow.expand.missingSubstitution"
    );
}

#[test]
fn validate_source_allows_template_placeholders() {
    validate_source(
        r#"
        use #{UsePin}
        Screen {
          #{PlaceholderPluginName}
        }
        "#,
    )
    .expect("templates should validate");
}

#[test]
fn fixture_showcase_expands_and_validates() {
    let source = include_str!("../../../fixtures/showcase/showcase.rbw");
    let map_json = include_str!("../../../fixtures/showcase/showcase.map.json");
    let map = parse_substitution_map(map_json).expect("map");
    let expanded = expand_and_validate(source, &map).expect("showcase expand");
    assert!(expanded.contains("use PromoBanner@1.0.0"));
    assert!(expanded.contains("@JSON({\"ok\":true"));
    assert!(!expanded.contains("#{"));
}

#[test]
fn lexer_scans_tagged_json_and_placeholder_blob() {
    let tokens = lex(r#"@JSON({"a":1}) @JSON(#{blob})"#).expect("lex");
    assert!(matches!(
        &tokens[0].kind,
        RainbowTokenKind::Tagged {
            language: EmbeddedLanguage::Json,
            body: RainbowTaggedBody::Text(text),
            ..
        } if text == r#"{"a":1}"#
    ));
    assert!(matches!(
        &tokens[1].kind,
        RainbowTokenKind::Tagged {
            language: EmbeddedLanguage::Json,
            body: RainbowTaggedBody::Placeholder(name),
            ..
        } if name == "blob"
    ));
}

#[test]
fn lexer_rejects_partial_placeholder_inside_tagged() {
    let error = lex(r#"@JSON({"a": #{x}})"#).expect_err("partial hole");
    assert_eq!(
        error.diagnostics[0].code,
        "rainbow.lexer.embeddedPartialPlaceholder"
    );
}

#[test]
fn lexer_keeps_at_for_use_pins() {
    let tokens = lex("use Screen@1.0.0").expect("lex");
    assert!(tokens
        .iter()
        .any(|t| matches!(t.kind, RainbowTokenKind::At)));
    assert!(!tokens
        .iter()
        .any(|t| matches!(t.kind, RainbowTokenKind::Tagged { .. })));
}

#[test]
fn lexer_emits_line_comment_tokens() {
    let tokens = lex("// pin\nuse Screen@1.0.0 // trailing\nScreen()").expect("lex");
    let comments: Vec<&str> = tokens
        .iter()
        .filter_map(|token| match &token.kind {
            RainbowTokenKind::Comment(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(comments, [" pin", " trailing"]);
    assert!(tokens
        .iter()
        .any(|t| matches!(&t.kind, RainbowTokenKind::Identifier(name) if name == "use")));
    assert!(tokens
        .iter()
        .any(|t| matches!(&t.kind, RainbowTokenKind::Identifier(name) if name == "Screen")));
}

#[test]
fn expand_works_with_utf8_comments() {
    let mut map = BTreeMap::new();
    map.insert("UsePin".to_string(), "PromoBanner@1.0.0".to_string());
    // Em dash in the comment must not shift placeholder byte offsets.
    let expanded = expand_placeholders("// pin — note\nuse #{UsePin}\n", &map).expect("expand");
    assert_eq!(expanded, "// pin — note\nuse PromoBanner@1.0.0\n");
}

#[test]
fn parser_rejects_unversioned_concrete_use() {
    let error = decode("use PromoBanner\nScreen()").expect_err("unversioned use");
    assert!(error
        .diagnostics
        .iter()
        .any(|d| d.message.contains("Unversioned")));
}

#[test]
fn parser_decodes_all_embedded_languages() {
    let document = decode(
        r#"
        Screen(
          json: @JSON({"key": "value"}),
          yaml: @YAML(enabled: true),
          xml: @XML(<user id="1"><name>Ada</name></user>),
          html: @HTML(<div class="card">Hi</div>),
          md: @MARKDOWN(# Title

Hello **world**),
          blob: @JSON(#{payload})
        )
        "#,
    )
    .expect("parse");

    let params = &document.nodes[0].parameters;
    assert_eq!(params.len(), 6);
    assert!(matches!(
        &params[0].value,
        RainbowValue::Tagged {
            language: EmbeddedLanguage::Json,
            body: RainbowTaggedBody::Text(_),
            ..
        }
    ));
    assert!(matches!(
        &params[5].value,
        RainbowValue::Tagged {
            language: EmbeddedLanguage::Json,
            body: RainbowTaggedBody::Placeholder(name),
            ..
        } if name == "payload"
    ));
}

#[test]
fn validate_rejects_invalid_embedded_json_with_body_range() {
    let error = validate_source(r#"Screen(json: @JSON({"a":))"#).expect_err("invalid json");
    let diagnostic = error
        .diagnostics
        .iter()
        .find(|d| d.code.starts_with("rainbow.embedded.json"))
        .expect("json diagnostic");
    assert!(diagnostic.range.start.offset > 0);
}

#[test]
fn validate_skips_embedded_placeholder_until_expand() {
    validate_source(r#"Screen(json: @JSON(#{payload}))"#).expect("template hole ok");
}

#[test]
fn expand_tagged_placeholder_wraps_language() {
    let mut map = BTreeMap::new();
    map.insert("payload".to_string(), r#"{"ok":true}"#.to_string());
    let expanded = expand_placeholders(r#"Screen(json: @JSON(#{payload}))"#, &map).expect("expand");
    assert_eq!(expanded, r#"Screen(json: @JSON({"ok":true}))"#);
    validate_concrete_source(&expanded).expect("concrete");
}

#[test]
fn format_pretty_prints_valid_json_body() {
    let formatted = encode(&decode(r#"Screen(json: @JSON({"a":1,"b":2}))"#).expect("decode"));
    assert!(formatted.contains("\"a\": 1"));
    assert!(formatted.contains("\"b\": 2"));
}

#[test]
fn format_preserves_leading_comments_and_blank_siblings() {
    let formatted = format_source(
        r#"
use Screen@1.0.0
// root comment
Screen() {
  Text(text: "a")
  // between plugins
  Button(title: "b")
}
"#,
    )
    .expect("format");

    assert!(formatted.contains("use Screen@1.0.0\n\n// root comment\nScreen {"));
    assert!(formatted.contains("Text(text: \"a\")\n\n  // between plugins\n  Button(title: \"b\")"));
}

#[test]
fn format_indents_multiline_json_and_breaks_params() {
    let formatted = format_source(
        r#"
SendHTTPRequest(url: "https://example.com", method: .POST, body: @JSON({"id":1}))
"#,
    )
    .expect("format");

    assert_eq!(
        formatted,
        r#"SendHTTPRequest(
  url: "https://example.com",
  method: .POST,
  body: @JSON({
    "id": 1
  })
)"#
    );
    validate_source(&formatted).expect("formatted validates");
}

#[test]
fn format_keeps_invalid_embedded_body() {
    // Invalid body still lexes/parses; encode must not invent pretty JSON.
    let source = r#"Screen(json: @JSON({not-json))"#;
    // `{not-json` is invalid JSON but still raw text for the scanner.
    let document = decode(source).expect("decode");
    let encoded = encode(&document);
    assert!(encoded.contains("@JSON({not-json)"));
    assert!(format_embedded(EmbeddedLanguage::Json, "{not-json").is_none());
}

#[test]
fn validate_embedded_yaml_xml_html_markdown() {
    validate_source(
        r#"
        Screen(
          yaml: @YAML(a: 1),
          xml: @XML(<root/>),
          html: @HTML(<p>hi</p>),
          md: @MARKDOWN(Hello)
        )
        "#,
    )
    .expect("valid embedded");

    let yaml_err = validate_source(r#"Screen(y: @YAML(: -))"#).expect_err("bad yaml");
    assert!(yaml_err
        .diagnostics
        .iter()
        .any(|d| d.code.starts_with("rainbow.embedded.yaml")));

    let xml_err = validate_source(r#"Screen(x: @XML(<root>))"#).expect_err("bad xml");
    assert!(xml_err
        .diagnostics
        .iter()
        .any(|d| d.code.starts_with("rainbow.embedded.xml")));

    let md_err = validate_source("Screen(m: @MARKDOWN(```\ncode\n))").expect_err("fence");
    assert!(md_err
        .diagnostics
        .iter()
        .any(|d| d.code == "rainbow.embedded.markdown.unclosedFence"));
}

#[test]
fn fixture_embedded_example_round_trips() {
    let source = include_str!("../../../fixtures/embedded/all-langs.rainbow");
    let document = decode(source).expect("fixture decode");
    let encoded = encode(&document);
    let again = decode(&encoded).expect("round-trip decode");
    assert_eq!(
        document.nodes[0].parameters.len(),
        again.nodes[0].parameters.len()
    );
    validate_source(&encoded).expect("formatted fixture validates");
}

#[test]
fn collect_embedded_regions_exports_bodies_and_flags() {
    let document = decode(
        r#"
        Screen(
          json: @JSON({"a":1}),
          hole: @YAML(#{cfg})
        )
        "#,
    )
    .expect("decode");
    let regions = collect_embedded_regions(&document);
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].language, EmbeddedLanguage::Json);
    assert!(!regions[0].is_placeholder);
    assert_eq!(regions[0].body, r#"{"a":1}"#);
    assert_eq!(regions[1].language, EmbeddedLanguage::Yaml);
    assert!(regions[1].is_placeholder);
    assert_eq!(regions[1].body, "#{cfg}");
}
