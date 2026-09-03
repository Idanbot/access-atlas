use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct GcloudFixture;

impl CommandRunner for GcloudFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "gcloud"
            && request.args == ["config", "configurations", "list", "--format=json"]
        {
            return Ok(CommandResult::success(
                r#"[{"name":"work","is_active":true,"properties":{"core":{"account":"operator@example.test","project":"demo-project"},"compute":{"region":"europe-west4","zone":"europe-west4-a"}}}]"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn local_gcloud_configuration_uses_project_and_zone_in_templates() {
    let report = DiscoveryService::new(
        GcloudFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let connection = report
        .inventory
        .connections
        .iter()
        .find(|connection| connection.provider == Provider::Gcloud)
        .expect("gcloud configuration should be discovered");

    assert_eq!(connection.label, "work");
    assert_eq!(connection.metadata["project"], "demo-project");
    assert_eq!(connection.metadata["zone"], "europe-west4-a");
    assert_eq!(connection.commands.len(), 10);
    let primary = connection.primary_commands();
    assert_eq!(
        primary
            .iter()
            .map(|command| command.kind)
            .collect::<Vec<_>>(),
        [
            ActionKind::Connect,
            ActionKind::PortForward,
            ActionKind::Debug,
        ]
    );
    assert!(primary.iter().all(|command| {
        command.command.contains("--configuration work")
            && command.command.contains("--project demo-project")
            && command.command.contains("--zone europe-west4-a")
    }));
}
