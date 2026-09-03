use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{fs, io};

#[derive(Clone)]
struct NoCommands;

impl CommandRunner for NoCommands {
    fn run(&self, _request: &CommandRequest) -> io::Result<CommandResult> {
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn terraform_discovers_only_explicit_roots_and_uses_infrastructure_actions() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let configured = sandbox.path().join("payments-infra");
    let unlisted = sandbox.path().join("secret-infra");
    fs::create_dir_all(configured.join(".terraform")).expect("configured root should exist");
    fs::create_dir_all(unlisted.join(".terraform")).expect("unlisted root should exist");
    fs::write(configured.join(".terraform/environment"), "production\n")
        .expect("workspace should be written");
    fs::write(unlisted.join(".terraform/environment"), "must-not-load\n")
        .expect("unlisted workspace should be written");

    let report = DiscoveryService::new(
        NoCommands,
        DiscoveryConfig::new(sandbox.path().to_owned(), vec![configured.clone()]),
    )
    .refresh(DiscoveryMode::Local);
    let terraform = report
        .inventory
        .connections
        .iter()
        .filter(|connection| connection.provider == Provider::Terraform)
        .collect::<Vec<_>>();

    assert_eq!(terraform.len(), 1);
    let connection = terraform[0];
    assert_eq!(connection.label, "payments-infra / production");
    assert_eq!(connection.metadata["workspace"], "production");
    assert_eq!(
        connection.metadata["root"],
        configured.display().to_string()
    );
    assert_eq!(connection.commands.len(), 10);
    assert!(connection.primary_commands().iter().all(|command| {
        command.kind == ActionKind::Inspect
            && command
                .command
                .contains(&format!("terraform -chdir={}", configured.display()))
    }));
    assert!(
        report
            .inventory
            .connections
            .iter()
            .all(|connection| !connection.label.contains("must-not-load"))
    );
}
