use pretty_assertions::assert_eq;
use tgg_model::Oid;

use super::*;

const C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const T: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

fn oid(hex: &str) -> Oid {
    Oid::parse(hex).unwrap()
}

fn annotated(name: &str, message: &str) -> String {
    format!(
        "refs/tags/{name}\x1ftag\x1f{T}\x1f{C}\x1fTag Ger\x1f<t@g.io>\x1f1793220853\x1f{message}\n\0\n"
    )
}

#[test]
fn args_ask_for_one_tag_with_nul_separated_records() {
    let args = args("v1.0");
    assert_eq!(args[0], "for-each-ref");
    assert_eq!(args[1], format!("--format={FORMAT}"));
    assert_eq!(&args[2..], ["--", "refs/tags/v1.0"]);
}

#[test]
fn annotated_tags_have_a_tagger_and_message() {
    let out = annotated("v1.0", "Release 1.0\n\nNotes:\n- fast");
    assert_eq!(
        parse(&out, "v1.0").unwrap(),
        Some(TagDetails {
            name: "v1.0".into(),
            commit: oid(C),
            annotation: Some(Annotation {
                tagger: Signature {
                    name: "Tag Ger".into(),
                    email: "t@g.io".into(),
                    time: 1_793_220_853,
                },
                message: "Release 1.0\n\nNotes:\n- fast".into(),
            }),
        })
    );
}

#[test]
fn lightweight_tags_have_no_annotation() {
    // %(contents) prints the commit message for a lightweight tag; ignore it.
    let out = format!("refs/tags/light\x1fcommit\x1f{C}\x1f\x1f\x1f\x1f\x1fcommit message\n\0\n");
    assert_eq!(
        parse(&out, "light").unwrap(),
        Some(TagDetails {
            name: "light".into(),
            commit: oid(C),
            annotation: None,
        })
    );
}

#[test]
fn only_the_exact_tag_is_returned() {
    // for-each-ref patterns also match as prefixes (`refs/tags/v1` matches `refs/tags/v1/x`).
    let out = format!("{}{}", annotated("v1/beta", "Beta"), annotated("v1", "One"));
    let tag = parse(&out, "v1").unwrap().unwrap();
    assert_eq!(tag.annotation.unwrap().message, "One");
    assert_eq!(parse(&out, "v2").unwrap(), None);
    assert_eq!(parse("", "v1").unwrap(), None);
}

#[test]
fn malformed_records_are_errors() {
    assert!(matches!(
        parse("refs/tags/v1\x1ftag\0", "v1"),
        Err(ParseError::MissingField { .. })
    ));
    let bad_time = format!("refs/tags/v1\x1ftag\x1f{C}\x1f{C}\x1fA\x1f<a@b>\x1fsoon\x1fm\0");
    assert!(matches!(
        parse(&bad_time, "v1"),
        Err(ParseError::InvalidNumber { .. })
    ));
}
