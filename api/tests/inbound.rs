use api::inbound::{self, Classification, InboundMessage};
use chrono::Utc;
use std::collections::BTreeMap;

fn message(headers: &[(&str, &str)]) -> InboundMessage {
    InboundMessage {
        provider_message_id: "m1".into(),
        thread_id: "t1".into(),
        from: "ada@example.com".into(),
        subject: "Re: Quick question".into(),
        headers: headers
            .iter()
            .map(|(key, value)| (key.to_lowercase(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
        received_at: Utc::now(),
    }
}

#[test]
fn a_person_answering_is_a_reply() {
    assert_eq!(
        inbound::classify(&message(&[("In-Reply-To", "<our-message@acme.com>")])),
        Classification::Reply
    );
}

#[test]
fn an_out_of_office_is_not_a_reply() {
    // Treating a vacation responder as engagement kills the sequence for
    // someone who never read it.
    for headers in [
        vec![("Auto-Submitted", "auto-replied")],
        vec![("Auto-Submitted", "auto-generated")],
        vec![("X-Autoreply", "yes")],
        vec![("X-Autorespond", "vacation")],
        vec![("Precedence", "auto_reply")],
    ] {
        assert_eq!(
            inbound::classify(&message(&headers)),
            Classification::AutoReply,
            "{headers:?} should be an auto-reply"
        );
    }
}

#[test]
fn a_delivery_failure_is_a_bounce() {
    let mut bounce = message(&[(
        "Content-Type",
        "multipart/report; report-type=delivery-status",
    )]);
    bounce.from = "mailer-daemon@googlemail.com".into();

    assert_eq!(inbound::classify(&bounce), Classification::Bounce);
}

#[test]
fn a_bounce_from_an_unusual_sender_is_still_caught_by_its_report_type() {
    let report = message(&[(
        "Content-Type",
        "multipart/report; report-type=delivery-status; boundary=x",
    )]);

    assert_eq!(inbound::classify(&report), Classification::Bounce);
}

#[test]
fn a_bounce_beats_an_auto_reply_header() {
    // Bounces often carry Auto-Submitted too; the failure is the important part.
    let mut bounce = message(&[
        ("Auto-Submitted", "auto-replied"),
        (
            "Content-Type",
            "multipart/report; report-type=delivery-status",
        ),
    ]);
    bounce.from = "MAILER-DAEMON@googlemail.com".into();

    assert_eq!(inbound::classify(&bounce), Classification::Bounce);
}
