//! Renders the commit details panel for a real commit into a buffer.

use ggt_git::GitCommand;
use ggt_git::show;
use ggt_model::Oid;
use ggt_testkit::FixtureRepo;
use ggt_ui::{CommitDetailsView, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn run(repo: &FixtureRepo, args: Vec<String>) -> String {
    let full = GitCommand::read(repo.path(), args).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    repo.git_raw(&full)
}

fn render(view: &CommitDetailsView, width: u16, height: u16) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf, &Theme::default());
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn details_of_a_commit_touching_nested_folders() {
    let repo = FixtureRepo::new();
    repo.commit_file("README.md", "# app\n", "Initial commit");
    repo.write_file("apps/full-app/src/main.rs", "fn main() {}\n");
    repo.write_file("apps/full-app/Cargo.toml", "[package]\n");
    repo.write_file("apps/web/index.html", "<html>\n");
    repo.write_file("README.md", "# app\n\nNow with apps.\n");
    repo.git(&["add", "--all"]);
    repo.git(&[
        "commit",
        "--quiet",
        "--message",
        "Add the apps",
        "--message",
        "Full app and a web page.",
    ]);
    let head = Oid::parse(&repo.git(&["rev-parse", "HEAD"])).unwrap();
    let details = show::parse_details(&run(&repo, show::details_args(&head))).unwrap();
    let files = show::parse_files(&run(&repo, show::files_args(&head))).unwrap();
    let view = CommitDetailsView::new(details, files);
    insta::assert_snapshot!(render(&view, 100, 10));
}
