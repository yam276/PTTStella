use super::*;
use fluent_syntax::{ast::*, parser};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn inline(value: &InlineExpression<&str>, variables: &mut BTreeSet<String>) {
    match value {
        InlineExpression::VariableReference { id } => {
            variables.insert(id.name.into());
        }
        InlineExpression::Placeable { expression: value } => expression(value, variables),
        InlineExpression::StringLiteral { .. } | InlineExpression::NumberLiteral { .. } => {}
        _ => panic!("catalog feature needs an explicit parity check: {value:?}"),
    }
}
fn expression(value: &Expression<&str>, variables: &mut BTreeSet<String>) {
    match value {
        Expression::Inline(value) => inline(value, variables),
        Expression::Select { selector, variants } => {
            inline(selector, variables);
            for variant in variants {
                pattern(&variant.value, variables);
            }
        }
    }
}
fn pattern(value: &Pattern<&str>, variables: &mut BTreeSet<String>) {
    for element in &value.elements {
        if let PatternElement::Placeable { expression: value } = element {
            expression(value, variables);
        }
    }
}
fn signature(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let parsed = parser::parse(source).expect("valid Fluent syntax, no junk");
    let mut messages = BTreeMap::new();
    for entry in parsed.body {
        if let Entry::Message(message) = entry {
            let mut variables = BTreeSet::new();
            pattern(&message.value.expect("message value"), &mut variables);
            assert!(message.attributes.is_empty());
            assert!(
                messages.insert(message.id.name.into(), variables).is_none(),
                "duplicate Fluent ID"
            );
        }
    }
    messages
}

#[test]
fn catalogs_have_exactly_the_same_ids_and_arguments_and_every_message_formats() {
    let expected = signature(Language::TraditionalChinese.resource());
    assert!(expected.len() > 250);
    assert_eq!(expected, signature(Language::English.resource()));
    for language in [Language::TraditionalChinese, Language::English] {
        let localizer = Localizer::new(language).unwrap();
        assert_eq!(localizer.language(), language);
        for (id, variables) in &expected {
            let args: HashMap<_, _> = variables
                .iter()
                .map(|name| (name.as_str(), "synthetic <script> & ID"))
                .collect();
            let text = localizer.loader().get_args(id, args);
            assert!(!text.is_empty(), "{id}");
            assert!(localizer.loader().has(id), "missing {id}");
        }
    }
}

#[test]
fn independent_loaders_keep_language_and_named_arguments_without_global_switching() {
    let chinese = Localizer::new(Language::TraditionalChinese).unwrap();
    let english = Localizer::new(Language::English).unwrap();
    assert_eq!(chinese.loader().get("connect"), "連線");
    assert_eq!(english.loader().get("connect"), "Connect");
    assert_eq!(
        i18n_embed_fl::fl!(english.loader(), "paste-title", paste_lines = "3"),
        "Paste 3 lines of text"
    );
    assert_eq!(
        english
            .loader()
            .get_args("filters-count-hidden", HashMap::from([("count", 1)])),
        "One reply hidden"
    );
    assert_eq!(
        english
            .loader()
            .get_args("filters-count-hidden", HashMap::from([("count", 2)])),
        "2 replies hidden"
    );
    assert_eq!(
        chinese.loader().get_args(
            "link-hover-hint",
            HashMap::from([("url", "https://example.com/<safe> & x")])
        ),
        "https://example.com/<safe> & x — 點擊開啟；右鍵開啟操作選單"
    );
    assert_eq!(chinese.loader().get("connect"), "連線");
    assert!(serde_json::from_str::<Language>("\"unsupported\"").is_err());
}
