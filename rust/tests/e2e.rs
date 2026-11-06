//! End-to-end tests: run the real binary with `--replay` fixtures.
//!
//! The unit tests exercise the decision trees directly; these check the parts
//! only the process can get wrong — argument handling, fixture loading, the
//! exact RPC sequence written to `--trace`, stdout/stderr text and exit codes.
//! They need no herdr server, which matters: the socket may be unreachable
//! (elevated server, CI, a sealed sandbox).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the binary under test, provided by cargo.
const BIN: &str = env!("CARGO_BIN_EXE_herdr-splits");

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A scratch directory under the crate's own `target/`.
    ///
    /// `std::env::temp_dir()` is not usable here: on Windows a sandboxed or
    /// service-account child process can be denied writes to `%TEMP%` even
    /// though the parent could write there. `target/` is always writable
    /// (cargo writes to it), gitignored, and cleaned by `cargo clean`.
    /// `HERDR_SPLITS_TEST_TMP` overrides the root for a read-only checkout.
    fn new(tag: &str) -> TempDir {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = match std::env::var_os("HERDR_SPLITS_TEST_TMP") {
            Some(override_root) if !override_root.is_empty() => PathBuf::from(override_root),
            _ => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("e2e-tmp"),
        };
        let path = root.join(format!("{tag}-{}-{serial}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)
            .unwrap_or_else(|err| panic!("create temp dir {}: {err}", path.display()));
        TempDir { path }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, contents).expect("write temp file");
        path
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A scratch conf nobody else uses, so a test that expects "no conf" really has
/// none instead of reading (and having created) the host's. Unique per call:
/// cargo runs one binary's tests in parallel threads, so a shared path would let
/// one test's cleanup delete a conf another test had just created.
fn sandbox_conf() -> String {
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = match std::env::var_os("HERDR_SPLITS_TEST_TMP") {
        Some(override_root) if !override_root.is_empty() => PathBuf::from(override_root),
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("e2e-tmp"),
    };
    let dir = root.join(format!("sandbox-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("herdr-splits-{serial}.conf"))
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(BIN);
    command.args(args);
    // Start from a clean slate: the host may be inside a real herdr session.
    for name in [
        "HERDR_SOCKET_PATH",
        "HERDR_SOCKET",
        "HERDR_PANE_ID",
        "HERDR_SPLITS_CONFIG",
        "HERDR_PLUGIN_CONFIG_DIR",
        "XDG_CONFIG_HOME",
        "APPDATA",
    ] {
        command.env_remove(name);
    }
    // `nav`/`resize`/`install` create the conf when it is missing, so keep them
    // away from the host's real config dir unless the test names its own below.
    let sandbox = sandbox_conf();
    command.env("HERDR_SPLITS_CONFIG", &sandbox);
    let overrides_config = env
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("HERDR_SPLITS_CONFIG"));
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run binary");
    // The path is unique to this call, so removing it cannot race a sibling test;
    // the directory goes only once it is empty.
    if !overrides_config {
        let _ = std::fs::remove_file(&sandbox);
        if let Some(parent) = Path::new(&sandbox).parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn trace_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// The trace format is `method<TAB>params<TAB>ok|error`; tests compare the
/// method sequence, which is the part the Python tests also pinned.
fn traced_methods(path: &Path) -> Vec<String> {
    trace_lines(path)
        .iter()
        .filter_map(|line| line.split('\t').next().map(str::to_string))
        .collect()
}

const PLAIN_DETECTION: &str = "\
{\"method\":\"pane.get\",\"response\":{\"result\":{\"pane\":{\"pane_id\":\"A\"}}}}
{\"method\":\"pane.process_info\",\"response\":{\"result\":{\"process_info\":{\"foreground_processes\":[{\"name\":\"bash\"}]}}}}
";

const EDGES_AT_LEFT: &str = "{\"method\":\"pane.edges\",\"response\":{\"result\":{\"edges\":{\"pane_id\":\"A\",\"left\":true,\"right\":false,\"up\":true,\"down\":true,\"layout\":{\"zoomed\":false,\"panes\":[{\"pane_id\":\"A\",\"rect\":{\"x\":0,\"y\":0,\"width\":40,\"height\":20}},{\"pane_id\":\"B\",\"rect\":{\"x\":40,\"y\":0,\"width\":40,\"height\":20}}]}}}}}\n";

/// Two plain panes side by side; the MRU path reads `pane.layout` first.
const LAYOUT: &str = "{\"method\":\"pane.layout\",\"response\":{\"result\":{\"layout\":{\"zoomed\":false,\"panes\":[{\"pane_id\":\"A\",\"rect\":{\"x\":0,\"y\":0,\"width\":40,\"height\":20}},{\"pane_id\":\"B\",\"rect\":{\"x\":40,\"y\":0,\"width\":40,\"height\":20}}]}}}}
";

#[test]
fn version_and_help_succeed() {
    let version = run(&["--version"], &[]);
    assert!(version.status.success());
    assert!(
        stdout(&version).starts_with("herdr-splits "),
        "{}",
        stdout(&version)
    );

    let help = run(&["--help"], &[]);
    assert!(help.status.success());
    let text = stdout(&help);
    assert!(text.contains("nav"), "{text}");
    assert!(text.contains("resize"), "{text}");
}

#[test]
fn init_writes_a_scaffold_where_the_readers_look() {
    let dir = TempDir::new("init");
    let conf = dir.join("herdr-splits.conf");
    let conf_str = conf.to_str().unwrap();

    let output = run(&["init"], &[("HERDR_SPLITS_CONFIG", conf_str)]);
    assert!(output.status.success(), "{}", stderr(&output));
    let written = std::fs::read_to_string(&conf).expect("conf written");
    // Not a generated file: the user owns it from the first byte.
    assert!(!written.contains("herdr-splits-generated-v1"), "{written}");
    assert!(written.contains("unzoom_on_nav=false"), "{written}");

    // A second run refuses to replace it (exit 1 = environment refusal, not a
    // usage error; nothing is clobbered).
    let again = run(&["init"], &[("HERDR_SPLITS_CONFIG", conf_str)]);
    assert_eq!(again.status.code(), Some(1));
    assert!(
        stderr(&again).contains("already exists"),
        "{}",
        stderr(&again)
    );
    assert_eq!(std::fs::read_to_string(&conf).unwrap(), written);

    // `--print` writes to stdout and touches nothing.
    let other = dir.join("printed.conf");
    let printed = run(
        &["init", "--print"],
        &[("HERDR_SPLITS_CONFIG", other.to_str().unwrap())],
    );
    assert!(printed.status.success(), "{}", stderr(&printed));
    assert!(stdout(&printed).contains("nav_key_left=ctrl+h"));
    assert!(!other.exists());
}

#[test]
fn usage_errors_exit_2() {
    for args in [
        &["nav"][..],
        &["nav", "sideways"][..],
        &["resize"][..],
        &["resize", "left", "not-a-number"][..],
        &["nonsense"][..],
    ] {
        let output = run(args, &[]);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(!stderr(&output).is_empty(), "{args:?} printed nothing");
    }
}

/// Without a socket and without a pane id, the run must fail the way the
/// Python script does: one line on stderr, exit 1.
#[test]
fn unreachable_socket_exits_1_with_the_python_wording() {
    let output = run(
        &["nav", "left"],
        &[("HERDR_SOCKET_PATH", "/nonexistent/herdr.sock")],
    );
    // Windows named pipes fail to open too, so this holds on both platforms.
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stderr(&output);
    assert!(
        text.contains("cannot connect to herdr socket"),
        "unexpected stderr: {text}"
    );
    assert!(text.contains("/nonexistent/herdr.sock"), "{text}");
}

#[test]
fn nav_forwards_the_chord_into_an_editor_pane() {
    let dir = TempDir::new("nav-editor");
    let fixture = dir.write(
        "fixture.jsonl",
        &[
            "{\"method\":\"pane.get\",\"response\":{\"result\":{\"pane\":{\"tokens\":{\"editor\":\"nvim\"}}}}}",
            "{\"method\":\"pane.send_keys\",\"response\":{\"result\":{}}}",
        ]
        .join("\n"),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "nav",
            "left",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[("HERDR_PANE_ID", "A")],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(traced_methods(&trace), vec!["pane.get", "pane.send_keys"]);
    let sent = trace_lines(&trace)
        .into_iter()
        .find(|line| line.starts_with("pane.send_keys"))
        .unwrap();
    assert!(sent.contains("\"keys\":[\"ctrl+h\"]"), "{sent}");
}

#[test]
fn nav_wraps_at_the_edge_when_the_conf_says_wrap() {
    let dir = TempDir::new("nav-wrap");
    let conf = dir.write(
        "herdr-splits.conf",
        "nav_at_edge=wrap\nmost_recently_focused=false\n",
    );
    let fixture = dir.write(
        "fixture.jsonl",
        &format!(
            "{PLAIN_DETECTION}{EDGES_AT_LEFT}{}",
            "{\"method\":\"pane.focus_direction\",\"response\":{\"result\":{}}}"
        ),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "nav",
            "left",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[
            ("HERDR_PANE_ID", "A"),
            ("HERDR_SPLITS_CONFIG", conf.to_str().unwrap()),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        traced_methods(&trace),
        vec![
            "pane.get",
            "pane.process_info",
            "pane.edges",
            "pane.focus_direction"
        ]
    );
    let focus = trace_lines(&trace)
        .into_iter()
        .find(|line| line.starts_with("pane.focus_direction"))
        .unwrap();
    assert!(focus.contains("\"direction\":\"right\""), "{focus}");
}

#[test]
fn nav_wraps_at_the_edge_by_default() {
    let dir = TempDir::new("nav-default-wrap");
    // No conf for this test: the sandbox path is missing, so the built-in
    // defaults decide — and they are `nav_at_edge=wrap` + `most_recently_focused`
    // on, so the MRU search already wraps to the pane at the opposite boundary
    // (here: B, to the right) without ever asking for `pane.edges`.
    let conf = dir.join("never-written-here.conf");
    let fixture = dir.write(
        "fixture.jsonl",
        &format!(
            "{PLAIN_DETECTION}{LAYOUT}{}",
            "{\"method\":\"pane.focus\",\"response\":{\"result\":{}}}"
        ),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "nav",
            "left",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[
            ("HERDR_PANE_ID", "A"),
            ("HERDR_SPLITS_CONFIG", conf.to_str().unwrap()),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        traced_methods(&trace),
        vec!["pane.get", "pane.process_info", "pane.layout", "pane.focus"]
    );
    let focus = trace_lines(&trace)
        .into_iter()
        .find(|line| line.starts_with("pane.focus"))
        .unwrap();
    assert!(focus.contains("\"pane_id\":\"B\""), "{focus}");
}

#[test]
fn nav_creates_the_conf_on_first_use() {
    let dir = TempDir::new("nav-ensure");
    let conf = dir.join("nested/herdr-splits.conf");
    let fixture = dir.write(
        "fixture.jsonl",
        &format!(
            "{PLAIN_DETECTION}{LAYOUT}{}",
            "{\"method\":\"pane.focus\",\"response\":{\"result\":{}}}"
        ),
    );

    assert!(!conf.exists());
    let output = run(
        &["nav", "left", "--replay", fixture.to_str().unwrap()],
        &[
            ("HERDR_PANE_ID", "A"),
            ("HERDR_SPLITS_CONFIG", conf.to_str().unwrap()),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));

    let written = std::fs::read_to_string(&conf).expect("conf created by nav");
    // The scaffold spells out the defaults, so creating it cannot change
    // behaviour — and it carries no generated marker: the user owns it.
    assert!(!written.contains("herdr-splits-generated-v1"), "{written}");
    assert!(written.contains("nav_at_edge=wrap"), "{written}");
    assert!(written.contains("most_recently_focused=true"), "{written}");
}

#[test]
fn nav_stops_at_the_edge_when_the_conf_says_stop() {
    let dir = TempDir::new("nav-stop");
    let conf = dir.write(
        "herdr-splits.conf",
        "nav_at_edge=stop\nmost_recently_focused=false\n",
    );
    let fixture = dir.write(
        "fixture.jsonl",
        &format!("{PLAIN_DETECTION}{EDGES_AT_LEFT}"),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "nav",
            "left",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[
            ("HERDR_PANE_ID", "A"),
            ("HERDR_SPLITS_CONFIG", conf.to_str().unwrap()),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        traced_methods(&trace),
        vec!["pane.get", "pane.process_info", "pane.edges"]
    );
}

#[test]
fn nav_pane_only_never_forwards_a_chord() {
    let dir = TempDir::new("nav-pane-only");
    // MRU off: this case is about the pane-level edges path, and the MRU path
    // would need a `pane.layout` reply first.
    let conf = dir.write("herdr-splits.conf", "most_recently_focused=false\n");
    let fixture = dir.write(
        "fixture.jsonl",
        &format!(
            "{EDGES_AT_LEFT}{}",
            "{\"method\":\"pane.focus_direction\",\"response\":{\"result\":{}}}"
        ),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "nav",
            "--pane-only",
            "right",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[
            ("HERDR_PANE_ID", "A"),
            ("HERDR_SPLITS_CONFIG", conf.to_str().unwrap()),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        traced_methods(&trace),
        vec!["pane.edges", "pane.focus_direction"]
    );
}

/// The caller's own edge policy wins over the shared conf: this is how a plain
/// Vim user gets `nav_at_edge=stop` without a Neovim setup() writing the conf.
#[test]
fn nav_pane_only_honours_the_nav_at_edge_override() {
    let dir = TempDir::new("nav-pane-only-stop");
    let conf = dir.write("herdr-splits.conf", "most_recently_focused=false\n");
    let fixture = dir.write("fixture.jsonl", EDGES_AT_LEFT);
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "nav",
            "--pane-only",
            "--nav-at-edge=stop",
            "left",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[
            ("HERDR_PANE_ID", "A"),
            ("HERDR_SPLITS_CONFIG", conf.to_str().unwrap()),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(traced_methods(&trace), vec!["pane.edges"]);
}

#[test]
fn resize_sends_the_amount_for_a_plain_pane() {
    let dir = TempDir::new("resize-plain");
    let fixture = dir.write(
        "fixture.jsonl",
        &format!(
            "{PLAIN_DETECTION}{}",
            "{\"method\":\"pane.resize\",\"response\":{\"result\":{}}}"
        ),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "resize",
            "right",
            "0.05",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[("HERDR_PANE_ID", "A")],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let resize = trace_lines(&trace)
        .into_iter()
        .find(|line| line.starts_with("pane.resize"))
        .unwrap();
    assert!(resize.contains("\"amount\":0.05"), "{resize}");
    assert!(resize.contains("\"direction\":\"right\""), "{resize}");
}

#[test]
fn resize_forwards_the_chord_into_nvim() {
    let dir = TempDir::new("resize-nvim");
    let fixture = dir.write(
        "fixture.jsonl",
        &[
            "{\"method\":\"pane.get\",\"response\":{\"result\":{\"pane\":{\"tokens\":{\"editor\":\"nvim\"}}}}}",
            "{\"method\":\"pane.send_keys\",\"response\":{\"result\":{}}}",
        ]
        .join("\n"),
    );
    let trace = dir.join("trace.log");

    let output = run(
        &[
            "resize",
            "up",
            "--replay",
            fixture.to_str().unwrap(),
            "--trace",
            trace.to_str().unwrap(),
        ],
        &[("HERDR_PANE_ID", "A")],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let sent = trace_lines(&trace)
        .into_iter()
        .find(|line| line.starts_with("pane.send_keys"))
        .unwrap();
    assert!(sent.contains("\"keys\":[\"alt+k\"]"), "{sent}");
}

/// A fixture that runs out is a test bug, not a runtime condition: the run must
/// fail loudly rather than silently do nothing.
#[test]
fn an_unscripted_rpc_call_fails_the_run() {
    let dir = TempDir::new("unscripted");
    let fixture = dir.write("fixture.jsonl", "");
    let output = run(
        &["nav", "left", "--replay", fixture.to_str().unwrap()],
        &[("HERDR_PANE_ID", "A")],
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("pane"), "{}", stderr(&output));
}

#[test]
fn doctor_reports_paths_without_a_server() {
    let output = run(
        &["doctor"],
        &[("HERDR_SOCKET_PATH", "/nonexistent/herdr.sock")],
    );
    // `doctor` is diagnostic: an unreachable server is reported, not fatal.
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("socket: /nonexistent/herdr.sock"), "{text}");
    assert!(text.contains("config:"), "{text}");
}

#[test]
fn install_writes_an_extensionless_binary() {
    let dir = TempDir::new("install");
    let root = dir.join("plugin");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("herdr-plugin.toml"), "id = \"herdr-splits\"\n").unwrap();

    let output = run(&["install", "--root", root.to_str().unwrap()], &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let destination = root.join("scripts/bin/herdr-splits");
    assert!(destination.is_file(), "{}", stdout(&output));

    // And the copy is itself runnable.
    let version = Command::new(&destination)
        .arg("--version")
        .output()
        .expect("run installed binary");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("herdr-splits "));
}

#[test]
fn install_without_a_checkout_is_a_clear_failure() {
    let dir = TempDir::new("install-missing-root");
    // A directory without the manifest: refuse rather than scatter files.
    let output = run(
        &["install", "--root", dir.join("nope").to_str().unwrap()],
        &[],
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("is not a plugin checkout"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn doctor_reports_where_the_policy_came_from() {
    let dir = TempDir::new("doctor-source");
    let conf = dir.join("herdr-splits.conf");
    let conf_str = conf.to_str().unwrap();

    // 1. Missing: documented as harmless, not as an error.
    let missing = run(&["doctor"], &[("HERDR_SPLITS_CONFIG", conf_str)]);
    let text = stdout(&missing) + &stderr(&missing);
    assert!(text.contains("config source: missing"), "{text}");

    // 2. Hand-written.
    std::fs::write(&conf, "unzoom_on_nav=true\n").unwrap();
    let written = run(&["doctor"], &[("HERDR_SPLITS_CONFIG", conf_str)]);
    let text = stdout(&written) + &stderr(&written);
    assert!(text.contains("config source: hand-written"), "{text}");

    // 3. Generated by an opt-in `setup()`.
    std::fs::write(&conf, "# herdr-splits-generated-v1\nunzoom_on_nav=true\n").unwrap();
    let generated = run(&["doctor"], &[("HERDR_SPLITS_CONFIG", conf_str)]);
    let text = stdout(&generated) + &stderr(&generated);
    assert!(text.contains("config source: generated by"), "{text}");
}

#[test]
fn a_config_dir_that_is_not_writable_is_not_a_usage_error() {
    let dir = TempDir::new("init-readonly");
    // A directory where the conf should be a file: creating it must fail.
    let conf = dir.join("nested/herdr-splits.conf");
    std::fs::create_dir_all(&conf).unwrap();

    let output = run(
        &["init"],
        &[("HERDR_SPLITS_CONFIG", conf.to_str().unwrap())],
    );
    // 1 = environment failure; 2 stays reserved for usage errors.
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(!stderr(&output).is_empty());
}

#[test]
fn doctor_distinguishes_an_unreadable_conf_from_a_missing_one() {
    let dir = TempDir::new("doctor-unreadable");
    let conf = dir.join("herdr-splits.conf");
    // A directory where a file is expected: reading fails with something that is
    // not NotFound, and must not be reported as the documented "missing" state.
    std::fs::create_dir_all(&conf).unwrap();

    let output = run(
        &["doctor"],
        &[("HERDR_SPLITS_CONFIG", conf.to_str().unwrap())],
    );
    let text = stdout(&output) + &stderr(&output);
    assert!(text.contains("config source: unreadable"), "{text}");
    assert!(!text.contains("config source: missing"), "{text}");
}
