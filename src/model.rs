use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, Deserialize)]
pub struct Topology {
    pub schema_version: u32,
    pub name: String,
    pub generated_at: String,
    pub origin: Origin,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Origin {
    pub id: String,
    pub label: String,
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub provider: String,
    pub location: Location,
    pub status: Health,
    pub network: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
    pub network_types: Vec<NetworkType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub label: String,
    pub region: String,
    pub city: String,
    pub country: String,
    pub timezone: String,
    pub source: String,
    pub precision: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Health {
    pub state: String,
    pub uptime_seconds: u64,
    pub latency_ms: f64,
    pub packet_loss_percent: f64,
    pub checked_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkType {
    pub id: String,
    pub label: String,
    pub binary: String,
    pub description: String,
    pub access_options: Vec<AccessOption>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccessOption {
    pub id: String,
    pub label: String,
    pub command: String,
    pub route: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailRow {
    pub label: String,
    pub value: String,
}

impl Topology {
    pub fn from_json(input: &str) -> Result<Self> {
        let topology: Self = serde_json::from_str(input).context("parse topology JSON")?;
        topology.validate()?;
        Ok(topology)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let input = fs::read_to_string(path)
            .with_context(|| format!("read topology JSON from {}", path.display()))?;
        Self::from_json(&input)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported topology schema version {}",
            self.schema_version
        );
        ensure!(
            !self.name.trim().is_empty(),
            "topology name must not be empty"
        );
        ensure!(
            !self.targets.is_empty(),
            "topology must contain at least one target"
        );
        validate_location(&self.origin.location, "origin")?;

        let mut target_ids = BTreeSet::new();
        for target in &self.targets {
            ensure!(!target.id.trim().is_empty(), "target id must not be empty");
            ensure!(
                target_ids.insert(&target.id),
                "duplicate target id: {}",
                target.id
            );
            validate_location(&target.location, &target.id)?;
            ensure!(
                !target.network_types.is_empty(),
                "target {} must have a network type",
                target.id
            );

            let mut network_type_ids = BTreeSet::new();
            for network_type in &target.network_types {
                ensure!(
                    !network_type.id.trim().is_empty(),
                    "network type id must not be empty"
                );
                ensure!(
                    network_type_ids.insert(&network_type.id),
                    "duplicate network type {} on target {}",
                    network_type.id,
                    target.id
                );
                ensure!(
                    !network_type.label.trim().is_empty(),
                    "network type label must not be empty"
                );
                ensure!(
                    !network_type.binary.trim().is_empty(),
                    "network type binary must not be empty"
                );
                ensure!(
                    !network_type.description.trim().is_empty(),
                    "network type description must not be empty"
                );
                ensure!(
                    !network_type.access_options.is_empty(),
                    "network type {} on target {} must have an access option",
                    network_type.id,
                    target.id
                );

                let mut option_ids = BTreeSet::new();
                for option in &network_type.access_options {
                    ensure!(
                        !option.id.trim().is_empty(),
                        "access option id must not be empty"
                    );
                    ensure!(
                        option_ids.insert(&option.id),
                        "duplicate access option {} on network type {}",
                        option.id,
                        network_type.id
                    );
                    ensure!(
                        !option.label.trim().is_empty(),
                        "access option label must not be empty"
                    );
                    ensure!(
                        !option.command.trim().is_empty(),
                        "access command must not be empty"
                    );
                    ensure!(!option.route.is_empty(), "access route must not be empty");
                    ensure!(
                        !option.notes.trim().is_empty(),
                        "access notes must not be empty"
                    );
                }
            }
        }
        Ok(())
    }
}

impl Target {
    pub fn detail_rows(&self) -> Vec<DetailRow> {
        let mut rows = vec![
            DetailRow {
                label: "id".to_owned(),
                value: self.id.clone(),
            },
            DetailRow {
                label: "provider".to_owned(),
                value: self.provider.clone(),
            },
            DetailRow {
                label: "kind".to_owned(),
                value: self.kind.clone(),
            },
            DetailRow {
                label: "location".to_owned(),
                value: format!(
                    "{}, {} ({})",
                    self.location.city, self.location.country, self.location.label
                ),
            },
            DetailRow {
                label: "region".to_owned(),
                value: self.location.region.clone(),
            },
            DetailRow {
                label: "timezone".to_owned(),
                value: self.location.timezone.clone(),
            },
            DetailRow {
                label: "location_source".to_owned(),
                value: format!("{} ({})", self.location.source, self.location.precision),
            },
            DetailRow {
                label: "status".to_owned(),
                value: self.status.state.clone(),
            },
            DetailRow {
                label: "uptime".to_owned(),
                value: format_uptime(self.status.uptime_seconds),
            },
            DetailRow {
                label: "latency".to_owned(),
                value: format!("{:.0} ms", self.status.latency_ms),
            },
            DetailRow {
                label: "packet_loss".to_owned(),
                value: format!("{:.1}%", self.status.packet_loss_percent),
            },
            DetailRow {
                label: "checked_at".to_owned(),
                value: self.status.checked_at.clone(),
            },
        ];

        rows.extend(self.network.iter().map(|(key, value)| DetailRow {
            label: format!("network.{}", key),
            value: value.clone(),
        }));
        rows.extend(self.metadata.iter().map(|(key, value)| DetailRow {
            label: format!("metadata.{}", key),
            value: value.clone(),
        }));
        rows
    }
}

fn validate_location(location: &Location, owner: &str) -> Result<()> {
    for (field, value) in [
        ("label", location.label.as_str()),
        ("region", location.region.as_str()),
        ("city", location.city.as_str()),
        ("country", location.country.as_str()),
        ("timezone", location.timezone.as_str()),
        ("source", location.source.as_str()),
        ("precision", location.precision.as_str()),
    ] {
        ensure!(
            !value.trim().is_empty(),
            "{} {} must not be empty",
            owner,
            field
        );
    }
    ensure!(
        (-90.0..=90.0).contains(&location.latitude),
        "{} latitude is outside [-90, 90]",
        owner
    );
    ensure!(
        (-180.0..=180.0).contains(&location.longitude),
        "{} longitude is outside [-180, 180]",
        owner
    );
    Ok(())
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    format!("{}d {:02}h {:02}m", days, hours, minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../data/demo-topology.json");

    #[test]
    fn demo_fixture_is_valid_and_covers_access_providers() {
        let topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        assert_eq!(topology.targets.len(), 8);
        let providers: BTreeSet<_> = topology
            .targets
            .iter()
            .map(|target| target.provider.as_str())
            .collect();
        assert_eq!(
            providers,
            BTreeSet::from([
                "aws",
                "azure",
                "cloudflare",
                "gcp",
                "kubernetes",
                "tailscale"
            ])
        );
    }

    #[test]
    fn fixture_models_network_types_and_multiple_options() {
        let topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        let gcp = &topology.targets[0];
        let ssh = gcp
            .network_types
            .iter()
            .find(|network_type| network_type.id == "ssh")
            .expect("gcp should expose ssh");
        assert_eq!(ssh.binary, "ssh");
        assert_eq!(ssh.access_options.len(), 3);

        let kubernetes = &topology.targets[2].network_types[0];
        assert_eq!(kubernetes.binary, "kubectl");
        assert_ne!(kubernetes.binary, ssh.binary);
        assert_eq!(topology.targets[6].location.city, "Ashburn");
        assert_eq!(topology.targets[7].location.city, "Tokyo");
    }

    #[test]
    fn target_details_include_location_network_and_metadata() {
        let topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        let target = &topology.targets[0];
        let rows = target.detail_rows();
        assert!(rows.iter().any(|row| row.label == "region"));
        assert!(rows.iter().any(|row| row.label == "timezone"));
        assert!(rows.iter().any(|row| row.label == "location_source"));
        assert!(
            rows.iter()
                .any(|row| row.label == "network.vpc" && row.value == "demo-eu-vpc")
        );
        assert!(rows.iter().any(|row| row.label == "metadata.project_id"));
        assert!(
            rows.iter()
                .any(|row| row.label == "uptime" && row.value.starts_with("14d"))
        );
    }

    #[test]
    fn duplicate_target_ids_are_rejected() {
        let mut topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        topology.targets[1].id = topology.targets[0].id.clone();
        let error = topology.validate().expect_err("duplicate ids should fail");
        assert!(error.to_string().contains("duplicate target id"));
    }

    #[test]
    fn duplicate_network_type_ids_are_rejected() {
        let mut topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        topology.targets[0].network_types[1].id = topology.targets[0].network_types[0].id.clone();
        let error = topology
            .validate()
            .expect_err("duplicate network type ids should fail");
        assert!(error.to_string().contains("duplicate network type"));
    }

    #[test]
    fn invalid_coordinates_are_rejected() {
        let mut topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        topology.targets[0].location.latitude = 91.0;
        let error = topology
            .validate()
            .expect_err("invalid latitude should fail");
        assert!(error.to_string().contains("latitude"));
    }
}
