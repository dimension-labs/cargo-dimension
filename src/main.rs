//! Command line tool for creating a Wasm contract and tests for use on the Dimension Platform.

#![deny(warnings)]

pub mod common;
mod contract_package;
pub mod dependency;
mod makefile;
mod rust_toolchain;
mod tests_package;
mod travis_yml;

use std::{
    env,
    path::{Path, PathBuf},
};

use clap::{builder::ValueParser, crate_description, crate_name, crate_version, Arg, Command};
use once_cell::sync::Lazy;

const USAGE: &str = r#"cargo dimension [FLAGS] <path>
    cd <path>
    make prepare
    make test"#;

const ROOT_PATH_ARG_NAME: &str = "path";
const ROOT_PATH_ARG_VALUE_NAME: &str = "path";
const ROOT_PATH_ARG_HELP: &str = "Path to new folder for contract and tests";

const WORKSPACE_PATH_ARG_NAME: &str = "workspace-path";
const WORKSPACE_PATH_ARG_LONG: &str = "workspace-path";

const GIT_URL_ARG_NAME: &str = "git-url";
const GIT_URL_LONG: &str = "git-url";

const GIT_BRANCH_ARG_NAME: &str = "git-branch";
const GIT_BRANCH_LONG: &str = "git-branch";

const FAILURE_EXIT_CODE: i32 = 101;

static ARGS: Lazy<Args> = Lazy::new(Args::new);

/// Can be used (via hidden command line args) to specify a patch section for the dimension crates in
/// the generated Cargo.toml files.
#[derive(Debug)]
enum DimensionOverrides {
    /// The path to local copy of the dimension-node repository.
    WorkspacePath(PathBuf),
    /// The details of an online copy of the dimension-node repository.
    GitRepo { url: String, branch: String },
}

#[derive(Debug)]
struct Args {
    root_path: PathBuf,
    dimension_overrides: Option<DimensionOverrides>,
}

impl Args {
    fn new() -> Self {
        // If run normally, the args passed are 'cargo-dimension', '<target dir>'.  However, if run as
        // a cargo subcommand (i.e. cargo dimension <target dir>), then cargo injects a new arg:
        // 'cargo-dimension', 'dimension', '<target dir>'.  We need to filter this extra arg out.
        //
        // This yields the situation where if the binary receives args of 'cargo-dimension', 'dimension'
        // then it might be a valid call (not a cargo subcommand - the user entered
        // 'cargo-dimension dimension' meaning to create a target dir called 'dimension') or it might be an
        // invalid call (the user entered 'cargo dimension' with no target dir specified).  The latter
        // case is assumed as being more likely.
        let filtered_args_iter = env::args().enumerate().filter_map(|(index, value)| {
            if index == 1 && value.as_str() == "dimension" {
                None
            } else {
                Some(value)
            }
        });

        let root_path_arg = Arg::new(ROOT_PATH_ARG_NAME)
            .value_parser(ValueParser::path_buf())
            .required(true)
            .value_name(ROOT_PATH_ARG_VALUE_NAME)
            .help(ROOT_PATH_ARG_HELP);

        let workspace_path_arg = Arg::new(WORKSPACE_PATH_ARG_NAME)
            .long(WORKSPACE_PATH_ARG_LONG)
            .takes_value(true)
            .hide(true);

        let git_url_arg = Arg::new(GIT_URL_ARG_NAME)
            .long(GIT_URL_LONG)
            .takes_value(true)
            .hide(true)
            .conflicts_with(WORKSPACE_PATH_ARG_NAME)
            .requires(GIT_BRANCH_ARG_NAME);

        let git_branch_arg = Arg::new(GIT_BRANCH_ARG_NAME)
            .long(GIT_BRANCH_LONG)
            .takes_value(true)
            .hide(true)
            .conflicts_with(WORKSPACE_PATH_ARG_NAME)
            .requires(GIT_URL_ARG_NAME);

        let arg_matches = Command::new(crate_name!())
            .version(crate_version!())
            .about(crate_description!())
            .override_usage(USAGE)
            .arg(root_path_arg)
            .arg(workspace_path_arg)
            .arg(git_url_arg)
            .arg(git_branch_arg)
            .get_matches_from(filtered_args_iter);

        let root_path = arg_matches
            .get_one::<PathBuf>(ROOT_PATH_ARG_NAME)
            .expect("expected path")
            .clone();

        let maybe_workspace_path = arg_matches.get_one::<String>(WORKSPACE_PATH_ARG_NAME);
        let maybe_git_url = arg_matches.get_one::<String>(GIT_URL_ARG_NAME);
        let maybe_git_branch = arg_matches.get_one::<String>(GIT_BRANCH_ARG_NAME);

        let dimension_overrides = match (maybe_workspace_path, maybe_git_url, maybe_git_branch) {
            (Some(path), None, None) => Some(DimensionOverrides::WorkspacePath(path.into())),
            (None, Some(url), Some(branch)) => Some(DimensionOverrides::GitRepo {
                url: url.to_string(),
                branch: branch.to_string(),
            }),
            (None, None, None) => None,
            _ => unreachable!("Clap rules enforce either both or neither git args are present"),
        };

        Args {
            root_path,
            dimension_overrides,
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn dimension_overrides(&self) -> Option<&DimensionOverrides> {
        self.dimension_overrides.as_ref()
    }
}

fn main() {
    if ARGS.root_path().exists() {
        common::print_error_and_exit(&format!(
            ": destination '{}' already exists",
            ARGS.root_path().display()
        ));
    }

    common::create_dir_all(ARGS.root_path());
    contract_package::create();
    tests_package::create();
    rust_toolchain::create();
    makefile::create();
    travis_yml::create();
}
