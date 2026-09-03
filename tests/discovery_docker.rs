use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct DockerFixture;

impl CommandRunner for DockerFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "docker"
            && request.args == ["context", "ls", "--format", "{{json .}}"]
        {
            return Ok(CommandResult::success(
                "{\"Name\":\"remote-prod\",\"Description\":\"Production engine\",\"DockerEndpoint\":\"ssh://docker.example.test\",\"Current\":true}\n",
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn docker_context_becomes_a_daemon_scoped_connection() {
    let report = DiscoveryService::new(
        DockerFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let connection = report
        .inventory
        .connections
        .iter()
        .find(|connection| connection.provider == Provider::Docker)
        .expect("Docker context should be discovered");

    assert_eq!(connection.label, "remote-prod");
    assert_eq!(connection.metadata["endpoint"], "ssh://docker.example.test");
    assert_eq!(connection.commands.len(), 10);
    assert_eq!(connection.primary_commands()[0].kind, ActionKind::Debug);
    assert!(
        connection
            .primary_commands()
            .iter()
            .all(|command| command.command.contains("docker --context remote-prod"))
    );
    assert!(
        connection
            .primary_commands()
            .iter()
            .all(|command| command.kind != ActionKind::Connect)
    );
}
