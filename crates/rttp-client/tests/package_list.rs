use std::process::Command;

#[test]
fn package_includes_client_test_files() {
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
    "tests/test_http_basic.rs",
    "tests/test_http_async.rs",
    "tests/test_response.rs",
    "tests/test_rustls.rs",
    "tests/test_raw_request_capture.rs",
    "tests/metadata_facade.rs",
  ] {
    assert!(
      package_files.lines().any(|line| line == expected),
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
