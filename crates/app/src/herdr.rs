//! herdr integration: plain right-click for the pane the app runs in.
//!
//! herdr sends plain right-click to its own pane menu by default, which
//! hides the app's context menus. herdr lets each pane route it to the app
//! instead (`herdr pane input --right-click pane`); [`RightClick`] does that
//! on startup and gives the click back to herdr on exit.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Environment variable that turns the routing off (`off`, `0`, `false`, `no`).
pub const RIGHT_CLICK_ENV: &str = "GIT_GRAPH_TUI_HERDR_RIGHT_CLICK";

/// How long one `herdr` call may take before it is given up on.
const CALL_TIMEOUT: Duration = Duration::from_millis(500);

/// Routes plain right-click to this app while it lives, inside herdr.
pub struct RightClick<F: Fn(&str, &[&str])> {
    herdr: String,
    pane: String,
    run: F,
}

impl<F: Fn(&str, &[&str])> RightClick<F> {
    /// Routes the pane's right-click to the app when running in a herdr pane
    /// and the routing isn't turned off. `run` runs one herdr command and
    /// ignores its outcome. None when there is nothing to route.
    pub fn enable(env: &dyn Fn(&str) -> Option<String>, run: F) -> Option<Self> {
        if env("HERDR_ENV").as_deref() != Some("1") || turned_off(env(RIGHT_CLICK_ENV)) {
            return None;
        }
        let present = |name| env(name).filter(|value| !value.trim().is_empty());
        let pane = present("HERDR_PANE_ID")?;
        let herdr = present("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".to_owned());
        let guard = Self { herdr, pane, run };
        guard.route("pane");
        Some(guard)
    }

    fn route(&self, target: &str) {
        let args = [
            "pane",
            "input",
            "--pane",
            &self.pane,
            "--right-click",
            target,
        ];
        (self.run)(&self.herdr, &args);
    }
}

impl<F: Fn(&str, &[&str])> Drop for RightClick<F> {
    fn drop(&mut self) {
        // herdr doesn't report the previous routing, so give back its default.
        self.route("herdr");
    }
}

fn turned_off(value: Option<String>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

/// Runs `program args` with no input or output, waiting at most
/// [`CALL_TIMEOUT`]. Failures are ignored: the routing is a convenience and
/// must never block the app.
pub fn run_quietly(program: &str, args: &[&str]) {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return;
    };
    let deadline = Instant::now() + CALL_TIMEOUT;
    while Instant::now() < deadline {
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use pretty_assertions::assert_eq;

    use super::*;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    const IN_HERDR: &[(&str, &str)] = &[
        ("HERDR_ENV", "1"),
        ("HERDR_PANE_ID", "w1:p2"),
        ("HERDR_BIN_PATH", "/opt/herdr"),
    ];

    /// Runs `f` with a recording runner and returns every call it made,
    /// including the ones made when the guard drops.
    fn calls(vars: &[(&str, &str)], f: impl FnOnce(bool)) -> Vec<String> {
        let log = RefCell::new(Vec::new());
        {
            let guard = RightClick::enable(&env(vars), |program: &str, args: &[&str]| {
                log.borrow_mut()
                    .push(format!("{program} {}", args.join(" ")));
            });
            f(guard.is_some());
        }
        log.into_inner()
    }

    #[test]
    fn routes_right_click_to_the_pane_and_gives_it_back_on_exit() {
        let mut during = Vec::new();
        let log = calls(IN_HERDR, |enabled| during.push(enabled));
        assert_eq!(during, [true]);
        assert_eq!(
            log,
            [
                "/opt/herdr pane input --pane w1:p2 --right-click pane",
                "/opt/herdr pane input --pane w1:p2 --right-click herdr",
            ]
        );
    }

    #[test]
    fn the_route_is_set_on_startup_before_the_app_runs() {
        let log = RefCell::new(Vec::new());
        let guard = RightClick::enable(&env(IN_HERDR), |program: &str, args: &[&str]| {
            log.borrow_mut()
                .push(format!("{program} {}", args.join(" ")));
        });
        assert_eq!(
            *log.borrow(),
            ["/opt/herdr pane input --pane w1:p2 --right-click pane"]
        );
        drop(guard);
        assert_eq!(log.borrow().len(), 2);
    }

    #[test]
    fn falls_back_to_herdr_on_the_path() {
        let log = calls(&[("HERDR_ENV", "1"), ("HERDR_PANE_ID", "w1:p2")], |_| {});
        assert_eq!(log[0], "herdr pane input --pane w1:p2 --right-click pane");
    }

    #[test]
    fn does_nothing_outside_herdr() {
        assert_eq!(
            calls(&[], |enabled| assert!(!enabled)),
            Vec::<String>::new()
        );
        let no_pane = [("HERDR_ENV", "1"), ("HERDR_BIN_PATH", "/opt/herdr")];
        assert!(calls(&no_pane, |_| {}).is_empty());
        let empty_pane = [("HERDR_ENV", "1"), ("HERDR_PANE_ID", " ")];
        assert!(calls(&empty_pane, |_| {}).is_empty());
        let not_one = [("HERDR_ENV", "0"), ("HERDR_PANE_ID", "w1:p2")];
        assert!(calls(&not_one, |_| {}).is_empty());
    }

    #[test]
    fn the_env_var_turns_it_off() {
        for off in ["off", "0", "false", "no", "OFF"] {
            let vars = [IN_HERDR, &[(RIGHT_CLICK_ENV, off)]].concat();
            assert!(calls(&vars, |_| {}).is_empty(), "{off:?}");
        }
        let on = [IN_HERDR, &[(RIGHT_CLICK_ENV, "on")]].concat();
        assert_eq!(calls(&on, |_| {}).len(), 2);
    }

    #[test]
    fn a_missing_herdr_binary_is_ignored() {
        // Must return promptly and not panic.
        run_quietly("/nonexistent/herdr", &["pane", "input"]);
    }

    #[test]
    fn a_hanging_herdr_is_given_up_on() {
        let start = Instant::now();
        run_quietly("sleep", &["5"]);
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
