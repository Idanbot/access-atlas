use access_atlas::discovery::{
    ActionKind, CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
    DiscoveryService, Provider,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct AwsOnlineFixture;

impl CommandRunner for AwsOnlineFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "aws" && request.args == ["configure", "list-profiles"] {
            return Ok(CommandResult::success("prod\n"));
        }
        if request.program == "aws"
            && request.args
                == [
                    "ec2",
                    "describe-instances",
                    "--profile",
                    "prod",
                    "--output",
                    "json",
                ]
        {
            return Ok(CommandResult::success(
                r#"{"Reservations":[{"Instances":[{"InstanceId":"i-0123456789abcdef0","State":{"Name":"running"},"Placement":{"AvailabilityZone":"eu-west-1a"},"PrivateIpAddress":"10.0.0.5","Tags":[{"Key":"Name","Value":"api-prod"}]}]}]}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "fixture missing"))
    }
}

#[test]
fn online_aws_refresh_adds_concrete_instance_commands() {
    let report = DiscoveryService::new(
        AwsOnlineFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Online);
    let instance = report
        .inventory
        .connections
        .iter()
        .find(|connection| {
            connection.provider == Provider::Aws && connection.kind == "ec2-instance"
        })
        .expect("online refresh should discover the EC2 instance");

    assert_eq!(instance.label, "api-prod");
    assert_eq!(instance.metadata["instance_id"], "i-0123456789abcdef0");
    assert_eq!(instance.metadata["profile"], "prod");
    assert_eq!(instance.metadata["region"], "eu-west-1");
    assert_eq!(instance.commands.len(), 10);
    assert_eq!(
        instance
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
    assert!(instance.primary_commands().iter().all(|command| {
        command.command.contains("i-0123456789abcdef0")
            && command.command.contains("--profile prod")
            && command.command.contains("--region eu-west-1")
    }));
}

#[derive(Clone)]
struct GcloudOnlineFixture;

impl CommandRunner for GcloudOnlineFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "gcloud"
            && request.args == ["config", "configurations", "list", "--format=json"]
        {
            return Ok(CommandResult::success(
                r#"[{"name":"work","is_active":true,"properties":{"core":{"project":"demo-project"},"compute":{"zone":"europe-west4-a"}}}]"#,
            ));
        }
        if request.program == "gcloud"
            && request.args
                == [
                    "--configuration",
                    "work",
                    "compute",
                    "instances",
                    "list",
                    "--project",
                    "demo-project",
                    "--format=json",
                ]
        {
            return Ok(CommandResult::success(
                r#"[{"name":"worker-1","zone":"https://www.googleapis.com/compute/v1/projects/demo-project/zones/europe-west4-a","status":"RUNNING","networkInterfaces":[{"networkIP":"10.0.0.6","accessConfigs":[{"natIP":"203.0.113.20"}]}]}]"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "fixture missing"))
    }
}

#[test]
fn online_gcloud_refresh_adds_concrete_compute_instance() {
    let report = DiscoveryService::new(
        GcloudOnlineFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Online);
    let instance = report
        .inventory
        .connections
        .iter()
        .find(|connection| {
            connection.provider == Provider::Gcloud && connection.kind == "compute-instance"
        })
        .expect("online refresh should discover the Compute Engine instance");

    assert_eq!(instance.label, "worker-1");
    assert_eq!(instance.metadata["project"], "demo-project");
    assert_eq!(instance.metadata["zone"], "europe-west4-a");
    assert_eq!(instance.metadata["external_ip"], "203.0.113.20");
    assert!(instance.primary_commands().iter().all(|command| {
        command.command.contains("worker-1")
            && command.command.contains("--project demo-project")
            && command.command.contains("--zone europe-west4-a")
    }));
}

#[derive(Clone)]
struct AzureOnlineFixture;

impl CommandRunner for AzureOnlineFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        let subscription = "00000000-0000-0000-0000-000000000001";
        if request.program == "az" && request.args == ["account", "list", "--output", "json"] {
            return Ok(CommandResult::success(format!(
                "[{{\"id\":\"{subscription}\",\"name\":\"Production\",\"isDefault\":true}}]"
            )));
        }
        if request.program == "az"
            && request.args
                == [
                    "vm",
                    "list",
                    "--show-details",
                    "--subscription",
                    subscription,
                    "--output",
                    "json",
                ]
        {
            return Ok(CommandResult::success(
                r#"[{"id":"/subscriptions/000/resourceGroups/platform/providers/Microsoft.Compute/virtualMachines/api-1","name":"api-1","resourceGroup":"platform","location":"westeurope","powerState":"VM running","privateIps":"10.0.0.7","publicIps":"203.0.113.30"}]"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "fixture missing"))
    }
}

#[test]
fn online_azure_refresh_adds_concrete_virtual_machine() {
    let report = DiscoveryService::new(
        AzureOnlineFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Online);
    let vm = report
        .inventory
        .connections
        .iter()
        .find(|connection| {
            connection.provider == Provider::Azure && connection.kind == "virtual-machine"
        })
        .expect("online refresh should discover the Azure VM");

    assert_eq!(vm.label, "api-1");
    assert_eq!(vm.metadata["resource_group"], "platform");
    assert_eq!(vm.metadata["location"], "westeurope");
    assert_eq!(vm.metadata["public_ip"], "203.0.113.30");
    assert!(vm.primary_commands().iter().all(|command| {
        command.command.contains("api-1")
            && command.command.contains("platform")
            && command
                .command
                .contains("00000000-0000-0000-0000-000000000001")
    }));
}
