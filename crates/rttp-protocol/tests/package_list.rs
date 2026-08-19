use std::process::Command;

#[test]
fn package_includes_protocol_metadata_facade_test() {
  let output = Command::new(env!("CARGO"))
    .arg("package")
    .arg("--list")
    .arg("--allow-dirty")
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .output()
    .expect("run cargo package --list");

  assert!(
    output.status.success(),
    "cargo package --list failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  let package_files = String::from_utf8(output.stdout).expect("package list is utf-8");
  for expected in [
    "README.md",
    "tests/link.rs",
    "tests/location.rs",
    "tests/max_forwards.rs",
    "tests/if_modified_since.rs",
    "tests/if_unmodified_since.rs",
    "tests/package_list.rs",
    "tests/metadata_facade.rs",
    "tests/content_disposition.rs",
    "tests/content_language.rs",
    "tests/no_vary_search.rs",
    "tests/upgrade.rs",
    "tests/accept_charset.rs",
    "tests/accept_language.rs",
  ] {
    assert!(
      package_files.lines().any(|path| path == expected),
      "missing {expected} from package list:\n{package_files}"
    );
  }
  assert!(
    package_files
      .lines()
      .all(|path| !path.contains("rttp-test-support")),
    "private test-support sources leaked into the package list:\n{package_files}"
  );
}
