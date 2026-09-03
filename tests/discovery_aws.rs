use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct AwsFixture;

impl CommandRunner for AwsFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "aws" && request.args == ["configure", "list-profiles"] {
            return Ok(CommandResult::success("dev\nprod\n"));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn local_aws_profiles_get_safe_profile_specific_commands() {
    let report = DiscoveryService::new(
        AwsFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let connections = report
        .inventory
        .connections
        .iter()
        .filter(|connection| connection.provider == Provider::Aws)
        .collect::<Vec<_>>();

    assert_eq!(connections.len(), 2);
    let prod = connections
        .iter()
        .find(|connection| connection.label == "prod")
        .expect("prod profile should be discovered");
    assert_eq!(prod.kind, "profile");
    assert_eq!(prod.commands.len(), 10);
    assert_eq!(prod.primary_commands().len(), 3);
    assert_eq!(prod.primary_commands()[0].kind, ActionKind::Debug);
    assert!(
        prod.primary_commands()
            .iter()
            .all(|command| command.command.contains("--profile prod"))
    );
    assert!(
        prod.primary_commands()
            .iter()
            .all(|command| command.kind != ActionKind::PortForward)
    );
}
