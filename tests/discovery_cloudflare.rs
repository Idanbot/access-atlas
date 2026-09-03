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
fn cloudflare_ingress_generates_service_aware_access_commands() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    fs::create_dir_all(sandbox.path().join(".cloudflared"))
        .expect("cloudflared directory should exist");
    fs::write(
        sandbox.path().join(".cloudflared/config.yml"),
        "tunnel: 11111111-1111-1111-1111-111111111111\ningress:\n  - hostname: ssh.example.test\n    service: ssh://localhost:22\n  - hostname: app.example.test\n    service: http://localhost:8080\n  - service: http_status:404\n",
    )
    .expect("cloudflared config should be written");

    let report = DiscoveryService::new(
        NoCommands,
        DiscoveryConfig::new(sandbox.path().to_owned(), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let connections = report
        .inventory
        .connections
        .iter()
        .filter(|connection| connection.provider == Provider::Cloudflare)
        .collect::<Vec<_>>();

    assert_eq!(connections.len(), 2);
    let ssh = connections
        .iter()
        .find(|connection| connection.label == "ssh.example.test")
        .expect("SSH ingress should be discovered");
    assert_eq!(ssh.metadata["service"], "ssh://localhost:22");
    assert!(!ssh.metadata.keys().any(|key| key.contains("credential")));
    assert_eq!(ssh.commands.len(), 10);
    assert_eq!(
        ssh.primary_commands()
            .iter()
            .map(|command| command.kind)
            .collect::<Vec<_>>(),
        [
            ActionKind::Connect,
            ActionKind::PortForward,
            ActionKind::Debug,
        ]
    );
    assert!(
        ssh.primary_commands()
            .iter()
            .all(|command| command.command.contains("ssh.example.test"))
    );
}
