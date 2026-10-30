//! `tag` parser run against output from a real git, on fixture repos.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use ggt_git::GitCommand;
use ggt_git::tag;
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

fn details(repo: &FixtureRepo, name: &str) -> Option<tag::TagDetails> {
    let full = GitCommand::read(repo.path(), tag::args(name)).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    tag::parse(&repo.git_raw(&full), name).unwrap()
}

#[test]
fn lightweight_and_annotated_tags() {
    let repo = FixtureRepo::sample();
    let light = details(&repo, "v0.1.0").unwrap();
    assert_eq!(light.commit.as_str(), repo.git(&["rev-parse", "v0.1.0"]));
    assert_eq!(light.annotation, None);

    repo.annotated_tag("v0.2.0", "Second release\n\n- multi-line notes\n- ünïcode");
    let annotated = details(&repo, "v0.2.0").unwrap();
    assert_eq!(
        annotated.commit.as_str(),
        repo.git(&["rev-parse", "v0.2.0^{commit}"])
    );
    let annotation = annotated.annotation.unwrap();
    assert_eq!(annotation.tagger.email, "fixture@example.com");
    assert_eq!(
        annotation.message,
        "Second release\n\n- multi-line notes\n- ünïcode"
    );

    assert_eq!(details(&repo, "missing"), None);
}
