use crate::{
    discovery::{CommandTemplate, ConnectionInventory, DiscoveredConnection, Provider},
    geo::Gazetteer,
    model::{AccessOption, Health, Location, MatchStatus, NetworkType, Target, Topology},
};
use std::collections::BTreeMap;

pub const DISCOVERED_PREFIX: &str = "discovered:";

pub fn discovered_target_id(connection_id: &str) -> String {
    format!("{DISCOVERED_PREFIX}{connection_id}")
}

pub fn connection_id_from_target(target_id: &str) -> Option<&str> {
    target_id.strip_prefix(DISCOVERED_PREFIX)
}

pub fn sync_topology(
    topology: &mut Topology,
    authored_count: usize,
    inventory: &ConnectionInventory,
    gazetteer: &Gazetteer,
) {
    topology.targets.truncate(authored_count);
    annotate_authored(&mut topology.targets, inventory);
    topology
        .targets
        .extend(inventory.connections.iter().map(|connection| {
            connection_as_target(connection, gazetteer, inventory.generated_at_unix)
        }));
}

fn annotate_authored(targets: &mut [Target], inventory: &ConnectionInventory) {
    for target in targets {
        if target.kind == "workstation" || target.id == "local-workstation" {
            target.match_status = MatchStatus::Source;
            continue;
        }
        let provider = Provider::parse(&target.provider);
        let matched = inventory.connections.iter().any(|connection| {
            Some(connection.provider) == provider
                && (connection.label.eq_ignore_ascii_case(&target.label)
                    || connection.id.contains(&target.id)
                    || target.id.contains(&connection.label))
        });
        target.match_status = if matched {
            MatchStatus::Matched
        } else {
            MatchStatus::Orphan
        };
    }
}

fn connection_as_target(
    connection: &DiscoveredConnection,
    gazetteer: &Gazetteer,
    generated_at_unix: u64,
) -> Target {
    let location = inferred_connection_location(connection, gazetteer);
    let primary = connection.primary_commands();
    let binary = primary
        .first()
        .and_then(|command| command.command.split_whitespace().next())
        .unwrap_or(connection.provider.as_str())
        .to_owned();
    let access_options = primary
        .into_iter()
        .map(|command| access_option(command, connection))
        .collect();
    let mut metadata = connection.metadata.clone();
    metadata.insert(
        "discovery.provider".to_owned(),
        connection.provider.as_str().to_owned(),
    );
    metadata.insert("discovery.kind".to_owned(), connection.kind.clone());
    metadata.insert(
        "match".to_owned(),
        format!("{:?}", MatchStatus::DiscoveredOnly).to_ascii_lowercase(),
    );

    Target {
        id: discovered_target_id(&connection.id),
        label: connection.label.clone(),
        kind: connection.kind.clone(),
        provider: connection.provider.as_str().to_owned(),
        location,
        status: Health {
            state: connection.health_state(),
            uptime_seconds: 0,
            latency_ms: 0.0,
            packet_loss_percent: 0.0,
            checked_at: format!("unix:{generated_at_unix}"),
            probed: false,
        },
        network: BTreeMap::from([("source".to_owned(), "local-cli".to_owned())]),
        metadata,
        match_status: MatchStatus::DiscoveredOnly,
        network_types: vec![NetworkType {
            id: "discovered-commands".to_owned(),
            label: format!("{} {}", connection.provider.as_str(), connection.kind),
            binary,
            description: "Read-only command templates generated from discovered metadata."
                .to_owned(),
            access_options,
        }],
    }
}

fn access_option(command: &CommandTemplate, connection: &DiscoveredConnection) -> AccessOption {
    AccessOption {
        id: command.id.clone(),
        label: command.label.clone(),
        command: command.command.clone(),
        route: vec![
            "local-workstation".to_owned(),
            connection.provider.as_str().to_owned(),
            connection.label.clone(),
        ],
        notes: command.description.clone(),
    }
}

fn inferred_connection_location(
    connection: &DiscoveredConnection,
    gazetteer: &Gazetteer,
) -> Location {
    match gazetteer.locate(connection) {
        Some(fix) => Location {
            label: format!("Estimated source · {}", fix.region),
            region: fix.region,
            city: fix.city,
            country: fix.country,
            timezone: "unknown".to_owned(),
            source: fix.source.to_owned(),
            precision: fix.source.to_owned(),
            latitude: fix.latitude,
            longitude: fix.longitude,
        },
        None => Location {
            label: "No location · source unknown".to_owned(),
            region: "none".to_owned(),
            city: "No location".to_owned(),
            country: "--".to_owned(),
            timezone: "unknown".to_owned(),
            source: "none".to_owned(),
            precision: "none".to_owned(),
            latitude: 0.0,
            longitude: 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{ActionKind, CommandTemplate, Provider};
    use crate::model::Topology;
    use std::collections::BTreeMap;

    const FIXTURE: &str = include_str!("../data/demo-topology.json");

    fn command(id: &str, command: &str) -> CommandTemplate {
        CommandTemplate {
            id: id.to_owned(),
            label: id.to_owned(),
            kind: ActionKind::Connect,
            command: command.to_owned(),
            description: "connect".to_owned(),
        }
    }

    fn connection(id: &str, label: &str, provider: Provider, region: &str) -> DiscoveredConnection {
        DiscoveredConnection {
            id: id.to_owned(),
            label: label.to_owned(),
            provider,
            kind: "profile".to_owned(),
            metadata: BTreeMap::from([
                ("profile".to_owned(), "prod".to_owned()),
                ("region".to_owned(), region.to_owned()),
                ("public_ip".to_owned(), "203.0.113.10".to_owned()),
            ]),
            commands: vec![command("ssh", "ssh prod")],
        }
    }

    #[test]
    fn sync_topology_matches_authored_targets_and_appends_discovered() {
        let mut topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        let authored = topology.targets.len();
        let inventory = ConnectionInventory {
            schema_version: 1,
            generated_at_unix: 1_700_000_000,
            connections: vec![connection(
                "aws:profile:prod",
                "AWS Europe runner",
                Provider::Aws,
                "eu-west-1",
            )],
        };

        sync_topology(&mut topology, authored, &inventory, &Gazetteer::default());

        let matched = topology
            .targets
            .iter()
            .find(|target| target.id == "aws-eu-runner")
            .expect("authored aws target remains");
        assert_eq!(matched.match_status, MatchStatus::Matched);

        let orphan = topology
            .targets
            .iter()
            .find(|target| target.id == "gcp-eu-micro")
            .expect("unmatched authored target remains");
        assert_eq!(orphan.match_status, MatchStatus::Orphan);

        let discovered = topology
            .targets
            .iter()
            .find(|target| target.id == "discovered:aws:profile:prod")
            .expect("discovered target should be appended");
        assert_eq!(discovered.match_status, MatchStatus::DiscoveredOnly);
        assert_eq!(discovered.status.state, "discovered");
        assert!(discovered.location_known());
        assert_eq!(
            connection_id_from_target(&discovered.id),
            Some("aws:profile:prod")
        );
        assert_eq!(
            discovered.network_types[0].access_options[0].command,
            "ssh prod"
        );
    }

    #[test]
    fn unknown_region_stays_unlocated() {
        let mut topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        let authored = topology.targets.len();
        let inventory = ConnectionInventory {
            schema_version: 1,
            generated_at_unix: 0,
            connections: vec![connection(
                "ssh:mystery",
                "mystery",
                Provider::Ssh,
                "not-a-region",
            )],
        };

        sync_topology(&mut topology, authored, &inventory, &Gazetteer::default());

        let discovered = topology
            .targets
            .iter()
            .find(|target| target.id == "discovered:ssh:mystery")
            .expect("discovered target");
        assert!(!discovered.location_known());
        assert_eq!(discovered.location.city, "No location");
    }
}
