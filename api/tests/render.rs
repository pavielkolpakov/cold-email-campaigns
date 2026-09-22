use api::render::{self, MergeValues};

fn values() -> MergeValues {
    MergeValues::from([
        ("first_name", "Ada"),
        ("company", "Analytical Engines"),
        ("title", "CTO"),
    ])
}

#[test]
fn merge_tags_are_replaced_with_lead_values() {
    let rendered = render::render("Hi {{first_name}}, how is {{company}}?", &values()).unwrap();

    assert_eq!(rendered, "Hi Ada, how is Analytical Engines?");
}

#[test]
fn a_fallback_fills_in_for_a_missing_value() {
    let rendered = render::render("Hi {{first_name|there}} at {{industry|your company}}", &values())
        .unwrap();

    assert_eq!(rendered, "Hi Ada at your company");
}

#[test]
fn an_empty_value_counts_as_missing() {
    let values = MergeValues::from([("first_name", "")]);

    let rendered = render::render("Hi {{first_name|there}}", &values).unwrap();

    assert_eq!(rendered, "Hi there");
}

#[test]
fn a_missing_value_with_no_fallback_refuses_to_render() {
    let values = MergeValues::from([("company", "Analytical Engines")]);

    let error = render::render("Hi {{first_name}} at {{company}}", &values).unwrap_err();

    assert_eq!(error, vec!["first_name"], "sending `Hi  at ...` is worse than not sending");
}

#[test]
fn every_unfilled_tag_is_reported_at_once() {
    let error = render::render("{{a}} {{b}} {{c|ok}}", &MergeValues::default()).unwrap_err();

    assert_eq!(error, vec!["a", "b"]);
}
