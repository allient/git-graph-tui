//! Renders a real file diff into a buffer.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tgg_git::GitCommand;
use tgg_git::show;
use tgg_model::Oid;
use tgg_testkit::FixtureRepo;
use tgg_ui::{DiffView, Theme};

fn run(repo: &FixtureRepo, args: Vec<String>) -> String {
    let full = GitCommand::read(repo.path(), args).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    repo.git_raw(&full)
}

#[test]
fn diff_of_a_modified_file() {
    let repo = FixtureRepo::new();
    repo.commit_file("src/lib.rs", "fn a() {}\nfn b() {}\nfn c() {}\n", "Base");
    repo.commit_file(
        "src/lib.rs",
        "fn a() {}\nfn b() -> u8 {\n\t1\n}\nfn c() {}\n",
        "Return a value",
    );
    let head = Oid::parse(&repo.git(&["rev-parse", "HEAD"])).unwrap();
    let file = show::parse_files(&run(&repo, show::files_args(&head)))
        .unwrap()
        .remove(0);
    let diff = show::parse_diff(&run(&repo, show::diff_args(&head, &file)));
    let view = DiffView::new(&file, diff);

    let area = Rect::new(0, 0, 50, 12);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf, &Theme::default());
    let text = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(text);
}
