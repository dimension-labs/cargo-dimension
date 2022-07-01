use std::{env, fs, process::Output};

use assert_cmd::Command;

const FAILURE_EXIT_CODE: i32 = 101;
const SUCCESS_EXIT_CODE: i32 = 0;
const TEST_PATH: &str = "test";
const GIT_URL_ARG: &str = "--git-url=https://github.com/dimension-labs/dimension-node";
/// GitHub Actions doesn't have good support for running scheduled jobs on non-default branches.
/// To work around this, our CI configuration will set an env var `BRANCH_SELECTOR` to the
/// appropriate branch name.  It will be unset on non-scheduled runs (e.g. merges, PRs).
const CRON_JOB_BRANCH_NAME_ENV_VAR: &str = "BRANCH_SELECTOR";
const PR_TARGET_BRANCH_NAME_ENV_VAR: &str = "GITHUB_BASE_REF";
const CI_BRANCH_NAME_ENV_VAR: &str = "GITHUB_REF_NAME";

#[test]
fn should_fail_when_target_path_already_exists() {
    let test_dir = tempfile::tempdir().unwrap().into_path();
    let output_error = Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg(&test_dir)
        .unwrap_err();

    let exit_code = output_error.as_output().unwrap().status.code().unwrap();
    assert_eq!(FAILURE_EXIT_CODE, exit_code);

    let stderr: String = String::from_utf8_lossy(&output_error.as_output().unwrap().stderr).into();
    let expected_msg_fragment = format!(": destination '{}' already exists", test_dir.display());
    assert!(stderr.contains(&expected_msg_fragment));
    assert!(stderr.contains("error"));

    fs::remove_dir_all(&test_dir).unwrap();
}

/// Runs `cmd` and returns the `Output` if successful, or panics on failure.
fn output_from_command(mut command: Command) -> Output {
    match command.ok() {
        Ok(output) => output,
        Err(error) => {
            panic!(
                "\nFailed to execute {:?}\n===== stderr begin =====\n{}\n===== stderr end \
                =====\n===== stdout begin =====\n{}\n===== stdout end =====\n",
                command,
                String::from_utf8_lossy(&error.as_output().unwrap().stderr),
                String::from_utf8_lossy(&error.as_output().unwrap().stdout)
            );
        }
    }
}

fn run_make_test_on_generated_project(maybe_git_branch_arg: Option<String>) {
    let temp_dir = tempfile::tempdir().unwrap().into_path();

    // Run 'cargo-dimension <test dir>/<subdir>'
    let subdir = TEST_PATH;
    let test_dir = temp_dir.join(subdir);
    let mut tool_cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    tool_cmd.arg(&test_dir);
    if let Some(git_branch_arg) = maybe_git_branch_arg {
        // Append '--git-url=...' and '--git-branch=...' args.
        tool_cmd.arg(GIT_URL_ARG);
        tool_cmd.arg(git_branch_arg);
    }

    // The CI environment doesn't have a Git user configured, so we can set the env var `USER` for
    // use by 'cargo new' which is called as a subprocess of 'cargo-dimension'.
    tool_cmd.env("USER", "tester");
    let tool_output = output_from_command(tool_cmd);
    assert_eq!(SUCCESS_EXIT_CODE, tool_output.status.code().unwrap());

    // Run 'make test' in the root of the generated project.  This builds the Wasm contract as well
    // as the tests.  This requires the use of a nightly version of Rust, so we use rustup to
    // execute the appropriate cargo version.
    let mut test_cmd = Command::new("rustup");
    let nightly_version = fs::read_to_string(format!(
        "{}/resources/rust-toolchain.in",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    test_cmd
        .arg("run")
        .arg(nightly_version.trim())
        .arg("make")
        .arg("test")
        .current_dir(test_dir);

    let test_output = output_from_command(test_cmd);
    assert_eq!(SUCCESS_EXIT_CODE, test_output.status.code().unwrap());

    // Cleans up temporary directory, but leaves it otherwise if the test failed.
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn ci_branch_name() -> Option<String> {
    if let Ok(branch_name) = env::var(CRON_JOB_BRANCH_NAME_ENV_VAR) {
        if !branch_name.is_empty() {
            return Some(branch_name);
        }
    }

    if let Ok(branch_name) = env::var(PR_TARGET_BRANCH_NAME_ENV_VAR) {
        if !branch_name.is_empty() {
            return Some(branch_name);
        }
    }

    if let Ok(branch_name) = env::var(CI_BRANCH_NAME_ENV_VAR) {
        if !branch_name.is_empty() {
            return Some(branch_name);
        }
    }

    None
}

/// Checks that running `cargo-dimension` with no specified overrides yields a generated project which
/// passes `make test`.
///
/// The generated project will have manifests which use the latest crates.io versions of the Dimension
/// dependencies.
///
/// If `BRANCH_SELECTOR`, `GITHUB_BASE_REF` or `GITHUB_REF_NAME` is set to `main`, the test is run.
/// If not, the test is an auto-pass.
#[test]
fn should_run_cargo_dimension_using_published_crates() {
    match ci_branch_name() {
        Some(branch_name) if branch_name == "main" => (),
        Some(branch_name) => {
            println!(
                "skipping 'should_run_cargo_dimension_using_published_crates' as branch name '{}' is \
                not 'main'",
                branch_name
            );
            return;
        }
        None => {
            println!(
                "skipping 'should_run_cargo_dimension_using_published_crates' as {} and {} are unset \
                or set to empty strings",
                PR_TARGET_BRANCH_NAME_ENV_VAR, CI_BRANCH_NAME_ENV_VAR
            );
            return;
        }
    }

    run_make_test_on_generated_project(None)
}

/// Checks that running `cargo-dimension` with Git overrides yields a generated project which passes
/// `make test`.
///
/// The generated project will have manifests which use Git overrides for the Dimension dependencies.
/// The versions will all be specified as `"*"` and the override branch of the `dimension-node` repo
/// will be as defined in the env var `BRANCH_SELECTOR` (for scheduled CI runs), `GITHUB_BASE_REF`
/// (for PRs) or `GITHUB_REF_NAME` (for any other CI runs) which is set by GitHub actions.
///
/// If none of `BRANCH_SELECTOR`, `GITHUB_BASE_REF` and `GITHUB_REF_NAME` are set, or they're all
/// set to empty strings, the test is an auto-pass.
///
/// For testing locally, you can manually run e.g.
/// ```
/// BRANCH_SELECTOR=dev cargo t
/// ```
#[test]
fn should_run_cargo_dimension_using_git_overrides() {
    // TODO - remove the following block.
    {
        let branch_selector = env::var(CRON_JOB_BRANCH_NAME_ENV_VAR);
        let pr_target_branch = env::var(PR_TARGET_BRANCH_NAME_ENV_VAR);
        let ci_branch = env::var(CI_BRANCH_NAME_ENV_VAR);
        println!("{}: {:?}", CRON_JOB_BRANCH_NAME_ENV_VAR, branch_selector);
        println!("{}: {:?}", PR_TARGET_BRANCH_NAME_ENV_VAR, pr_target_branch);
        println!("{}: {:?}", CI_BRANCH_NAME_ENV_VAR, ci_branch);
    }

    let git_branch_arg = match ci_branch_name() {
        Some(branch_name) if branch_name == "main" => {
            println!(
                "skipping 'should_run_cargo_dimension_using_git_overrides' as branch name is 'main'"
            );
            return;
        }
        Some(branch_name) => format!("--git-branch={}", branch_name),
        None => {
            println!(
                "skipping 'should_run_cargo_dimension_using_git_overrides' as {} and {} are unset or set \
            to empty strings",
                PR_TARGET_BRANCH_NAME_ENV_VAR, CI_BRANCH_NAME_ENV_VAR
            );
            return;
        }
    };

    run_make_test_on_generated_project(Some(git_branch_arg))
}
