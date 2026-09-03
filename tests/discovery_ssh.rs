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
fn ssh_config_imports_concrete_hosts_without_credentials() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    fs::create_dir_all(sandbox.path().join(".ssh")).expect("ssh directory should exist");
    fs::write(
        sandbox.path().join(".ssh/config"),
        "Host *\n  ServerAliveInterval 30\n\nHost jump-prod\n  HostName 203.0.113.10\n  User deploy\n  Port 2222\n  ProxyJump bastion\n  IdentityFile ~/.ssh/id_secret\n",
    )
    .expect("ssh config should be written");

    let report = DiscoveryService::new(
        NoCommands,
        DiscoveryConfig::new(sandbox.path().to_owned(), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);
    let ssh = report
        .inventory
        .connections
        .iter()
        .filter(|connection| connection.provider == Provider::Ssh)
        .collect::<Vec<_>>();

    assert_eq!(ssh.len(), 1);
    let connection = ssh[0];
    assert_eq!(connection.label, "jump-prod");
    assert_eq!(connection.metadata["hostname"], "203.0.113.10");
    assert_eq!(connection.metadata["user"], "deploy");
    assert_eq!(connection.metadata["port"], "2222");
    assert_eq!(connection.metadata["proxy_jump"], "bastion");
    assert!(!connection.metadata.contains_key("identity_file"));
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
            .all(|command| command.command.contains("jump-prod"))
    );
}
