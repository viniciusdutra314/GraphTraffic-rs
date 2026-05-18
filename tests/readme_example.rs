use assert_cmd::Command;
#[test]
fn readme_example_produces_hdf5() {
    let mut cmd = Command::cargo_bin("graph_traffic").unwrap();
    cmd.args(&["examples/config.json", "--force"])
        .assert()
        .success();
}
