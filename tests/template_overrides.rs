use access_atlas::discovery::{
    CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode, DiscoveryService,
};
use std::{fs, io};

#[derive(Clone)]
struct KubectlFixture;

impl CommandRunner for KubectlFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" {
            return Ok(CommandResult::success(
                r#"{"contexts":[{"name":"prod-eu","context":{"cluster":"prod","namespace":"payments"}}],"clusters":[]}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn valid_override_replaces_a_typed_command_and_interpolates_metadata() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let path = sandbox.path().join("templates.json");
    fs::write(
        &path,
        r#"{
          "version": 1,
          "overrides": [{
            "provider": "kubernetes",
            "resource_kind": "context",
            "id": "connect",
            "label": "Namespace overview",
            "action": "connect",
            "command": "kubectl --context {context} --namespace {namespace} get all",
            "description": "Inspect {namespace} through the selected context."
          }]
        }"#,
    )
    .expect("override");
    let config =
        DiscoveryConfig::new(sandbox.path().to_owned(), Vec::new()).with_template_overrides(path);

    let report = DiscoveryService::new(KubectlFixture, config).refresh(DiscoveryMode::Local);
    let connection = report
        .inventory
        .connections
        .iter()
        .find(|connection| connection.label == "prod-eu")
        .expect("connection");
    let command = connection
        .commands
        .iter()
        .find(|command| command.id == "connect")
        .expect("overridden command");

    assert_eq!(command.label, "Namespace overview");
    assert_eq!(
        command.command,
        "kubectl --context prod-eu --namespace payments get all"
    );
    assert!(report.notices.is_empty());
}

#[test]
fn invalid_override_preserves_the_builtin_and_reports_the_problem() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let path = sandbox.path().join("templates.json");
    fs::write(
        &path,
        r#"{"version":1,"overrides":[{
          "provider":"kubernetes",
          "resource_kind":"context",
          "id":"connect",
          "label":"Broken",
          "action":"connect",
          "command":"kubectl --context {missing_value} get pods",
          "description":"Broken placeholder"
        }]}"#,
    )
    .expect("override");
    let config =
        DiscoveryConfig::new(sandbox.path().to_owned(), Vec::new()).with_template_overrides(path);

    let report = DiscoveryService::new(KubectlFixture, config).refresh(DiscoveryMode::Local);
    let connection = report
        .inventory
        .connections
        .iter()
        .find(|connection| connection.label == "prod-eu")
        .expect("connection");

    assert!(connection.commands[0].command.contains("cluster-info"));
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.contains("missing_value"))
    );
}
