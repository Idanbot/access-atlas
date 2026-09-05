use access_atlas::discovery::{
    CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode, DiscoveryService,
    audit_refresh,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct KubectlFixture;

impl CommandRunner for KubectlFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" {
            return Ok(CommandResult::success(
                r#"{"contexts":[{"name":"audit-prod","context":{"cluster":"prod","namespace":"default"}}],"clusters":[]}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn acceptance_audit_validates_generated_commands_without_running_them() {
    let refresh = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);

    let audit = audit_refresh(&refresh);

    assert!(audit.passed, "issues: {:?}", audit.issues);
    assert_eq!(audit.connection_count, 1);
    assert_eq!(audit.command_count, 10);
    assert!(
        audit
            .warnings
            .iter()
            .any(|warning| warning.contains("unavailable"))
    );
}

#[test]
fn acceptance_audit_rejects_duplicate_ids_and_unsafe_command_lines() {
    let mut refresh = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let duplicate = refresh.inventory.connections[0].clone();
    refresh.inventory.connections.push(duplicate);
    refresh.inventory.connections[0].commands[0]
        .command
        .push_str("\nsecond command");

    let audit = audit_refresh(&refresh);

    assert!(!audit.passed);
    assert!(
        audit
            .issues
            .iter()
            .any(|issue| issue.contains("duplicate connection id"))
    );
    assert!(
        audit
            .issues
            .iter()
            .any(|issue| issue.contains("single line"))
    );
}

#[test]
fn acceptance_audit_rejects_credential_keys_placeholders_and_empty_commands() {
    let mut refresh = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    refresh.inventory.connections[0]
        .metadata
        .insert("session_token".to_owned(), "not-a-secret-value".to_owned());
    refresh.inventory.connections[0].commands[1].command =
        "kubectl get pods -n {namespace}".to_owned();
    refresh.inventory.connections[0].commands[2].command.clear();

    let audit = audit_refresh(&refresh);

    assert!(!audit.passed);
    assert!(
        audit
            .issues
            .iter()
            .any(|issue| issue.contains("prohibited credential metadata key"))
    );
    assert!(
        audit
            .issues
            .iter()
            .any(|issue| issue.contains("unresolved template placeholder"))
    );
    assert!(
        audit
            .issues
            .iter()
            .any(|issue| issue.contains("empty command"))
    );
}
