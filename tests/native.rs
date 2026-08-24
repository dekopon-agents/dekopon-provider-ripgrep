use dekopon_provider_sdk::{CapabilityId, Provider as _};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::{Value, json};

fn invoke(input: Value) -> Value {
    RipgrepProvider::invoke(
        &"ripgrep.search"
            .parse::<CapabilityId>()
            .expect("valid capability"),
        input,
    )
    .expect("search succeeds")
}

fn kinds(output: &Value) -> Vec<&str> {
    output["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|result| result["kind"].as_str().expect("kind string"))
        .collect()
}

#[test]
fn regex_fixed_order_offsets_and_overall_occurrences() {
    let output = invoke(json!({
        "documents": [
            {"path": "z/first", "text": "no\nfoo foo\n"},
            {"path": "a/second", "text": "foo.bar\nfooXbar\n"}
        ],
        "pattern": "fo+"
    }));
    assert_eq!(output["selected_count"], 3);
    assert_eq!(output["truncated"], false);
    let results = output["results"].as_array().expect("results");
    assert_eq!(results[0]["path"], "z/first");
    assert_eq!(results[0]["text"], "foo foo\n");
    assert_eq!(results[0]["byte_start"], 3);
    assert_eq!(results[0]["byte_end"], 11);
    assert_eq!(results[0]["line_start"], 2);
    assert_eq!(results[0]["line_end"], 2);
    assert_eq!(
        results[0]["submatches"],
        json!([
            {"byte_start": 3, "byte_end": 6},
            {"byte_start": 7, "byte_end": 10}
        ])
    );
    assert_eq!(results[1]["path"], "a/second");
    assert_eq!(results[2]["path"], "a/second");

    let fixed = invoke(json!({
        "documents": [{"path": "literal", "text": "foo.bar\nfooXbar\n"}],
        "pattern": "foo.bar",
        "mode": "fixed"
    }));
    assert_eq!(fixed["selected_count"], 1);
    assert_eq!(fixed["results"][0]["text"], "foo.bar\n");
    assert_eq!(
        fixed["results"][0]["submatches"],
        json!([{"byte_start": 0, "byte_end": 7}])
    );
}

#[test]
fn case_modes_word_and_whole_line_use_unicode_and_lf_semantics() {
    let documents =
        json!([{"path": "unicode", "text": "Δέλτα\nδΈΛΤΑ\nCAFÉ caféine café\nfoo\r\nfoo\n"}]);
    let insensitive = invoke(json!({
        "documents": documents,
        "pattern": "δέλτα",
        "case": "insensitive"
    }));
    assert_eq!(insensitive["selected_count"], 2);

    let smart_lower = invoke(json!({
        "documents": documents,
        "pattern": "δέλτα",
        "case": "smart"
    }));
    assert_eq!(smart_lower["selected_count"], 2);
    let smart_upper = invoke(json!({
        "documents": documents,
        "pattern": "Δέλτα",
        "case": "smart"
    }));
    assert_eq!(smart_upper["selected_count"], 1);

    let word = invoke(json!({
        "documents": documents,
        "pattern": "café",
        "case": "insensitive",
        "word": true
    }));
    assert_eq!(word["selected_count"], 1);
    assert_eq!(word["results"][0]["text"], "CAFÉ caféine café\n");
    assert_eq!(
        word["results"][0]["submatches"].as_array().unwrap().len(),
        2
    );

    let line = invoke(json!({
        "documents": documents,
        "pattern": "foo",
        "line": true
    }));
    assert_eq!(line["selected_count"], 1);
    assert_eq!(line["results"][0]["text"], "foo\n");
    assert_eq!(line["results"][0]["line_start"], 5);
}

#[test]
fn multiline_returns_complete_blocks_and_pattern_controls_dotall() {
    let output = invoke(json!({
        "documents": [{"path": "multi", "text": "pre\nfoo\nmiddle\nbar\npost\n"}],
        "pattern": "foo(?s:.*?)bar",
        "multiline": true
    }));
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"][0]["text"], "foo\nmiddle\nbar\n");
    assert_eq!(output["results"][0]["byte_start"], 4);
    assert_eq!(output["results"][0]["byte_end"], 19);
    assert_eq!(output["results"][0]["line_start"], 2);
    assert_eq!(output["results"][0]["line_end"], 4);
    assert_eq!(
        output["results"][0]["submatches"],
        json!([{"byte_start": 4, "byte_end": 18}])
    );

    let no_dotall = invoke(json!({
        "documents": [{"path": "multi", "text": "foo\nbar\n"}],
        "pattern": "foo.*bar",
        "multiline": true
    }));
    assert_eq!(no_dotall["selected_count"], 0);
}

#[test]
fn invert_and_context_are_ordered_deduplicated_and_prioritized() {
    let context = invoke(json!({
        "documents": [{"path": "ctx", "text": "zero\nA\nshared\nB\nlast\n"}],
        "pattern": "^(?:A|B)$",
        "context": {"before": 1, "after": 1}
    }));
    assert_eq!(context["selected_count"], 2);
    assert_eq!(
        kinds(&context),
        [
            "context_before",
            "match",
            "context_before",
            "match",
            "context_after"
        ]
    );
    assert_eq!(context["results"][2]["text"], "shared\n");
    assert_eq!(context["results"][2]["submatches"], json!([]));

    let inverted = invoke(json!({
        "documents": [{"path": "invert", "text": "hit\nmiss\nhit again\n"}],
        "pattern": "hit",
        "invert": true,
        "context": {"before": 1, "after": 1}
    }));
    assert_eq!(inverted["selected_count"], 1);
    assert_eq!(inverted["results"][1]["text"], "miss\n");
    assert_eq!(inverted["results"][1]["submatches"], json!([]));
    assert_eq!(inverted["results"][1]["submatches_truncated"], false);
}

#[test]
fn bom_cr_final_line_empty_document_zero_width_and_byte_mode_are_preserved() {
    let bom = invoke(json!({
        "documents": [{"path": "bytes", "text": "\u{feff}head\r\nfinal"}],
        "pattern": "\u{feff}head\\r$"
    }));
    assert_eq!(bom["selected_count"], 1);
    assert_eq!(bom["results"][0]["text"], "\u{feff}head\r\n");
    assert_eq!(bom["results"][0]["byte_start"], 0);
    assert_eq!(bom["results"][0]["byte_end"], 9);

    let final_line = invoke(json!({
        "documents": [
            {"path": "empty", "text": ""},
            {"path": "final", "text": "one\nlast"}
        ],
        "pattern": "last$"
    }));
    assert_eq!(final_line["selected_count"], 1);
    assert_eq!(final_line["results"][0]["text"], "last");
    assert_eq!(final_line["results"][0]["line_start"], 2);

    let zero_width = invoke(json!({
        "documents": [{"path": "zero", "text": "ab\ncd"}],
        "pattern": "^|$"
    }));
    assert_eq!(zero_width["selected_count"], 2);
    assert_eq!(
        zero_width["results"][0]["submatches"],
        json!([
            {"byte_start": 0, "byte_end": 0},
            {"byte_start": 2, "byte_end": 2}
        ])
    );
    assert_eq!(
        zero_width["results"][1]["submatches"],
        json!([
            {"byte_start": 3, "byte_end": 3},
            {"byte_start": 5, "byte_end": 5}
        ])
    );

    let byte_mode = invoke(json!({
        "documents": [{"path": "byte-mode", "text": "é\n"}],
        "pattern": "(?-u:\\xA9)"
    }));
    assert_eq!(
        byte_mode["results"][0]["submatches"],
        json!([{"byte_start": 1, "byte_end": 2}])
    );
}

#[test]
fn combining_marks_have_byte_offsets_not_character_offsets() {
    let output = invoke(json!({
        "documents": [{"path": "combining", "text": "e\u{0301} e\u{0301}x\n"}],
        "pattern": "e\u{0301}",
        "word": true
    }));
    assert_eq!(output["selected_count"], 1);
    assert_eq!(
        output["results"][0]["submatches"],
        json!([{"byte_start": 0, "byte_end": 3}])
    );
}
