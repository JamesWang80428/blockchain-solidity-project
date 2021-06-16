// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::UnitTestingConfig;

pub fn run_tests_with_config_and_filter(
    mut config: UnitTestingConfig,
    root_path: &str,
    source_pattern: &str,
    dep_root: Option<&str>,
) {
    let get_files = |root_path, pat| {
        let source_re = regex::Regex::new(pat)
            .unwrap_or_else(|_| panic!("Invalid regular expression: '{}'", pat));
        move_stdlib::utils::iterate_directory(&std::path::Path::new(root_path))
            .filter_map(|path| {
                let name = path.to_string_lossy();
                if source_re.is_match(&name) {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .collect()
    };

    let sources = get_files(root_path, source_pattern);
    let deps = dep_root
        .map(|root| get_files(root, r"*.move"))
        .unwrap_or_else(Vec::new);

    config.source_files = sources;
    config.dep_files = deps;
    let test_plan = config.build_test_plan().expect("Unable to build test plan");

    let (_, all_tests_passed) = config
        .run_and_report_unit_tests(test_plan, std::io::stdout())
        .expect("Failed to execute tests");

    // If all tests passed, exit with 0 otherwise with a non-zero exit code.
    if all_tests_passed {
        std::process::exit(0)
    } else {
        std::process::exit(1)
    }
}

#[macro_export]
macro_rules! register_move_unit_tests {
    ($config:expr, $root:expr, $pattern:expr) => {
        #[test]
        fn move_unit_tests() {
            $crate::cargo_runner::run_tests_with_config_and_filter($config, $root, $pattern, None)
        }
    };
    ($config:expr, $root:expr, $source_pattern:expr, $dep_root:expr) => {
        #[test]
        fn move_unit_tests() {
            $crate::cargo_runner::run_tests_with_config_and_filter(
                $config,
                $root,
                $source_pattern,
                Some($dep_root),
            )
        }
    };
}
