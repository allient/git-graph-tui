//! `show` parsers run against output from a real git, on fixture repos.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use pretty_assertions::assert_eq;
use tgg_git::GitCommand;
use tgg_git::show::{self, FileChange, LineKind, Numstat};
use tgg_model::Oid;
use tgg_testkit::FixtureRepo;

/// Runs `args` the way the app does (`-C`, `core.quotepath=off`, ...).
fn run(repo: &FixtureRepo, args: Vec<String>) -> String {
    let full = GitCommand::read(repo.path(), args).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    repo.git_raw(&full)
}

fn rev(repo: &FixtureRepo, name: &str) -> Oid {
    Oid::parse(&repo.git(&["rev-parse", name])).unwrap()
}

/// The first commit of `main`.
fn root(repo: &FixtureRepo) -> Oid {
    Oid::parse(&repo.git(&["rev-list", "--max-parents=0", "main"])).unwrap()
}

fn files(repo: &FixtureRepo, commit: &Oid) -> Vec<FileChange> {
    show::parse_files(&run(repo, show::files_args(commit))).unwrap()
}

fn paths(files: &[FileChange]) -> Vec<&str> {
    files.iter().map(|f| f.path.as_str()).collect()
}

fn lines(added: u32, deleted: u32) -> Numstat {
    Numstat::Lines { added, deleted }
}

#[test]
fn details_of_a_merge_and_a_root_commit() {
    let repo = FixtureRepo::sample();
    let merge = rev(&repo, "main");
    let details = show::parse_details(&run(&repo, show::details_args(&merge))).unwrap();
    assert_eq!(details.commit.id, merge);
    assert_eq!(details.commit.parents.len(), 2);
    assert_eq!(details.commit.subject, "Merge branch 'develop'");
    assert_eq!(details.commit.author.email, "fixture@example.com");

    let root = root(&repo);
    let details = show::parse_details(&run(&repo, show::details_args(&root))).unwrap();
    assert!(details.commit.parents.is_empty());
    assert_eq!(details.commit.subject, "Initial commit");
    assert_eq!(details.body(), "");
}

#[test]
fn details_keep_a_multi_line_body() {
    let repo = FixtureRepo::new();
    repo.write_file("a.txt", "a\n");
    repo.git(&["add", "a.txt"]);
    repo.git(&[
        "commit",
        "--quiet",
        "--message",
        "Subject line",
        "--message",
        "First paragraph\nwith two lines.\n\nSecond: tabs\tand ünïcode.",
    ]);
    let details =
        show::parse_details(&run(&repo, show::details_args(&rev(&repo, "HEAD")))).unwrap();
    assert_eq!(details.commit.subject, "Subject line");
    assert_eq!(
        details.body(),
        "First paragraph\nwith two lines.\n\nSecond: tabs\tand ünïcode."
    );
}

#[test]
fn root_commit_lists_every_file_as_added() {
    let repo = FixtureRepo::sample();
    let root = root(&repo);
    assert_eq!(
        files(&repo, &root),
        [FileChange {
            path: "README.md".into(),
            old_path: None,
            stat: lines(1, 0)
        }]
    );
}

#[test]
fn merge_lists_what_it_brought_in_from_the_first_parent() {
    let repo = FixtureRepo::sample();
    let merge = rev(&repo, "main");
    assert_eq!(paths(&files(&repo, &merge)), ["src/lib.rs"]);
}

#[test]
fn renames_binaries_and_unusual_paths() {
    let repo = FixtureRepo::new();
    repo.commit_file("old name.txt", "one\ntwo\nthree\nfour\n", "Base");
    std::fs::write(repo.path().join("logo.png"), [0u8, 159, 146, 150]).unwrap();
    repo.write_file("dir é/tab\tand space.md", "日本語\n");
    repo.git(&["mv", "old name.txt", "new name.txt"]);
    repo.write_file("new name.txt", "one\ntwo\nthree\nfour\nfive\n");
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "--quiet", "--message", "Mixed"]);

    let mut changes = files(&repo, &rev(&repo, "HEAD"));
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(
        changes,
        [
            FileChange {
                path: "dir é/tab\tand space.md".into(),
                old_path: None,
                stat: lines(1, 0)
            },
            FileChange {
                path: "logo.png".into(),
                old_path: None,
                stat: Numstat::Binary
            },
            FileChange {
                path: "new name.txt".into(),
                old_path: Some("old name.txt".into()),
                stat: lines(1, 0)
            },
        ]
    );

    // Each file's diff, through the same FileChange.
    let diff_of = |path: &str| {
        let file = changes.iter().find(|f| f.path == path).unwrap();
        show::parse_diff(&run(&repo, show::diff_args(&rev(&repo, "HEAD"), file)))
    };
    let renamed = diff_of("new name.txt");
    assert_eq!((renamed.added(), renamed.deleted()), (1, 0));
    assert!(
        renamed
            .lines
            .iter()
            .any(|l| l.kind == LineKind::Header && l.text == "rename from old name.txt")
    );
    let added = renamed
        .lines
        .iter()
        .find(|l| l.kind == LineKind::Added)
        .unwrap();
    assert_eq!((added.text.as_str(), added.new_line), ("+five", Some(5)));

    assert!(diff_of("logo.png").is_binary());

    let unicode = diff_of("dir é/tab\tand space.md");
    assert_eq!(unicode.added(), 1);
    assert!(unicode.lines.iter().any(|l| l.text == "+日本語"));
}

#[test]
fn diff_of_a_file_in_the_root_commit() {
    let repo = FixtureRepo::sample();
    let root = root(&repo);
    let file = files(&repo, &root).remove(0);
    let diff = show::parse_diff(&run(&repo, show::diff_args(&root, &file)));
    assert_eq!(diff.added(), 1);
    assert!(diff.lines.iter().any(|l| l.text == "--- /dev/null"));
}

#[test]
fn diff_of_a_merge_is_against_the_first_parent() {
    let repo = FixtureRepo::sample();
    let merge = rev(&repo, "main");
    let file = files(&repo, &merge).remove(0);
    let diff = show::parse_diff(&run(&repo, show::diff_args(&merge, &file)));
    assert_eq!(diff.added(), 1);
    assert!(diff.lines.iter().any(|l| l.text == "+pub fn lib() {}"));
}

#[test]
fn pathspec_characters_in_names_are_literal() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "a\n", "Base");
    repo.write_file("star*.txt", "s\n");
    repo.write_file("starry.txt", "other\n");
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "--quiet", "--message", "Stars"]);
    let head = rev(&repo, "HEAD");
    let star = files(&repo, &head)
        .into_iter()
        .find(|f| f.path == "star*.txt")
        .unwrap();
    let diff = show::parse_diff(&run(&repo, show::diff_args(&head, &star)));
    assert_eq!(diff.added(), 1, "only star*.txt, not starry.txt");
}

#[test]
fn file_at_a_revision_is_its_content_then() {
    let repo = FixtureRepo::new();
    let first = Oid::parse(&repo.commit_file("src/a b.txt", "one\ntwo\n", "Base")).unwrap();
    repo.commit_file("src/a b.txt", "changed\n", "Change");
    assert_eq!(
        show::file_at_args(&first, "src/a b.txt"),
        ["show".to_owned(), format!("{first}:src/a b.txt")]
    );
    assert_eq!(
        run(&repo, show::file_at_args(&first, "src/a b.txt")),
        "one\ntwo\n"
    );
}
