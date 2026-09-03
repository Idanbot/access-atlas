use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct TailscaleFixture;

impl CommandRunner for TailscaleFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "tailscale" && request.args == ["status", "--json"] {
            return Ok(CommandResult::success(
                r#"{"Peer":{"nodekey:peer":{"HostName":"db-prod","DNSName":"db-prod.example.ts.net.","TailscaleIPs":["100.64.0.10"],"Online":true,"OS":"linux"}}}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn tailscale_peers_use_dns_name_for_access_templates() {
    let report = DiscoveryService::new(
        TailscaleFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let connection = report
        .inventory
        .connections
        .iter()
        .find(|connection| connection.provider == Provider::Tailscale)
        .expect("Tailscale peer should be discovered");

    assert_eq!(connection.label, "db-prod");
    assert_eq!(connection.metadata["address"], "db-prod.example.ts.net");
    assert_eq!(connection.metadata["ip"], "100.64.0.10");
    assert_eq!(connection.metadata["online"], "true");
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
    assert!(
        connection
            .primary_commands()
            .iter()
            .all(|command| command.command.contains("db-prod.example.ts.net"))
    );
}
