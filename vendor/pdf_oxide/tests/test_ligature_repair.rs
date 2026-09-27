use pdf_oxide::converters::text_post_processor::TextPostProcessor;

#[test]
fn all_five_ligature_substitutions_between_letters() {
    assert_eq!(TextPostProcessor::repair_ligatures("di!erent"), "different");
    assert_eq!(TextPostProcessor::repair_ligatures("o\"ce"), "office");
    assert_eq!(TextPostProcessor::repair_ligatures("de#ne"), "define");
    assert_eq!(TextPostProcessor::repair_ligatures("re$ect"), "reflect");
    assert_eq!(TextPostProcessor::repair_ligatures("ba%e"), "baffle");
}

#[test]
fn substitution_requires_preceding_letter() {
    assert_eq!(TextPostProcessor::repair_ligatures("#nancial"), "#nancial");
}

#[test]
fn punctuation_preserved_at_word_boundaries() {
    assert_eq!(TextPostProcessor::repair_ligatures("Hello!"), "Hello!");
    assert_eq!(TextPostProcessor::repair_ligatures("$100"), "$100");
    assert_eq!(TextPostProcessor::repair_ligatures("50%"), "50%");
    assert_eq!(TextPostProcessor::repair_ligatures("\"hello\""), "\"hello\"");
}

#[test]
fn mixed_broken_ligatures_and_real_punctuation() {
    assert_eq!(
        TextPostProcessor::repair_ligatures("di!erent o\"ces #nancial"),
        "different offices #nancial"
    );
}

// --- Leader dot normalization ---

#[test]
fn four_or_more_dots_collapsed_to_ellipsis() {
    assert_eq!(
        TextPostProcessor::normalize_leader_dots("Section ............. 5"),
        "Section ... 5"
    );
}

#[test]
fn three_or_fewer_dots_preserved() {
    assert_eq!(TextPostProcessor::normalize_leader_dots("wait..."), "wait...");
    assert_eq!(TextPostProcessor::normalize_leader_dots("hmm.."), "hmm..");
    assert_eq!(TextPostProcessor::normalize_leader_dots("one."), "one.");
}
