use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct KubectlFixture;

impl CommandRunner for KubectlFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" && request.args == ["config", "view", "-o", "json"] {
            return Ok(CommandResult::success(
                r#"{
                  "current-context": "prod-eu",
                  "contexts": [{
                    "name": "prod-eu",
                    "context": {
                      "cluster": "prod-eu-cluster",
                      "namespace": "payments",
                      "user": "oidc-user"
                    }
                  }],
                  "clusters": [{
                    "name": "prod-eu-cluster",
                    "cluster": {"server": "https://api.prod.example.test"}
                  }]
                }"#,
            ));
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("fixture has no command: {request:?}"),
        ))
    }
}

#[test]
fn local_kubectl_context_becomes_a_templated_connection() {
    let config = DiscoveryConfig::new(PathBuf::from("/tmp/access-atlas-missing-home"), Vec::new());
    let report = DiscoveryService::new(KubectlFixture, config).refresh(DiscoveryMode::Local);

    assert_eq!(report.inventory.connections.len(), 1);
    let connection = &report.inventory.connections[0];
    assert_eq!(connection.provider, Provider::Kubernetes);
    assert_eq!(connection.kind, "context");
    assert_eq!(connection.label, "prod-eu");
    assert_eq!(connection.metadata["cluster"], "prod-eu-cluster");
    assert_eq!(connection.metadata["namespace"], "payments");
    assert_eq!(connection.commands.len(), 10);

    let primary = connection.primary_commands();
    assert_eq!(primary.len(), 3);
    assert_eq!(primary[0].kind, ActionKind::Connect);
    assert_eq!(primary[1].kind, ActionKind::PortForward);
    assert_eq!(primary[2].kind, ActionKind::Debug);
    assert!(
        primary
            .iter()
            .all(|command| command.command.contains("--context prod-eu"))
    );
    assert!(
        primary
            .iter()
            .all(|command| command.command.contains("--namespace payments"))
    );
}
