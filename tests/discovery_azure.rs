use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct AzureFixture;

impl CommandRunner for AzureFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "az" && request.args == ["account", "list", "--output", "json"] {
            return Ok(CommandResult::success(
                r#"[{"id":"00000000-0000-0000-0000-000000000001","name":"Production","tenantId":"00000000-0000-0000-0000-000000000002","isDefault":true,"state":"Enabled"}]"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn local_azure_subscription_scopes_primary_vm_templates() {
    let report = DiscoveryService::new(
        AzureFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let connection = report
        .inventory
        .connections
        .iter()
        .find(|connection| connection.provider == Provider::Azure)
        .expect("Azure subscription should be discovered");

    assert_eq!(connection.label, "Production");
    assert_eq!(connection.kind, "subscription");
    assert_eq!(connection.metadata["state"], "Enabled");
    assert_eq!(connection.commands.len(), 10);
    assert_eq!(
        connection
            .primary_commands()
            .iter()
            .map(|command| command.kind)
            .collect::<Vec<_>>(),
        [
            ActionKind::Connect,
            ActionKind::PortForward,
            ActionKind::Debug,
        ]
    );
    assert!(connection.primary_commands().iter().all(|command| {
        command
            .command
            .contains("--subscription 00000000-0000-0000-0000-000000000001")
    }));
}
