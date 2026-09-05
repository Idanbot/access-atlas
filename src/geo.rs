use crate::discovery::DiscoveredConnection;
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct GeoLocation {
    pub city: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    pub region: String,
    pub source: &'static str,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct GazetteerFile {
    #[serde(default)]
    regions: BTreeMap<String, GeoFix>,
    #[serde(default)]
    hosts: BTreeMap<String, GeoFix>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeoFix {
    city: String,
    country: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Gazetteer {
    overrides: GazetteerFile,
}

impl Gazetteer {
    pub fn load(path: &Path) -> Self {
        let Ok(input) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self {
            overrides: serde_json::from_str(&input).unwrap_or_default(),
        }
    }

    pub fn locate(&self, connection: &DiscoveredConnection) -> Option<GeoLocation> {
        for key in [
            Some(connection.label.as_str()),
            connection.metadata.get("hostname").map(String::as_str),
            connection.metadata.get("name").map(String::as_str),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(fix) = self.overrides.hosts.get(key) {
                return Some(owned_fix(fix, key, "operator-host"));
            }
        }

        let raw = connection
            .metadata
            .get("region")
            .or_else(|| connection.metadata.get("location"))
            .or_else(|| connection.metadata.get("zone"))
            .map(String::as_str)?;
        let code = normalize_region(raw);

        if let Some(fix) = self.overrides.regions.get(&code) {
            return Some(owned_fix(fix, &code, "operator-region"));
        }
        builtin(&code).map(|(city, country, latitude, longitude)| GeoLocation {
            city: city.to_owned(),
            country: country.to_owned(),
            latitude,
            longitude,
            region: code,
            source: "estimated-region",
        })
    }
}

fn owned_fix(fix: &GeoFix, region: &str, source: &'static str) -> GeoLocation {
    GeoLocation {
        city: fix.city.clone(),
        country: fix.country.clone(),
        latitude: fix.latitude,
        longitude: fix.longitude,
        region: region.to_owned(),
        source,
    }
}

pub fn normalize_region(code: &str) -> String {
    let trimmed = code.trim().to_ascii_lowercase();
    let base = trimmed.rsplit('/').next().unwrap_or(&trimmed);
    if let Some((region, zone)) = base.rsplit_once('-')
        && zone.len() == 1
        && zone
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return region.to_owned();
    }
    if base
        .chars()
        .last()
        .is_some_and(|character| character.is_ascii_alphabetic())
        && let Some(stripped) = base.strip_suffix(|character: char| character.is_ascii_alphabetic())
        && stripped
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return stripped.to_owned();
    }
    base.to_owned()
}

fn builtin(code: &str) -> Option<(&'static str, &'static str, f64, f64)> {
    for (region, city, country, latitude, longitude) in REGIONS {
        if *region == code {
            return Some((city, country, *latitude, *longitude));
        }
    }
    None
}

const REGIONS: &[(&str, &str, &str, f64, f64)] = &[
    ("africa-south1", "Johannesburg", "ZA", -26.20, 28.04),
    ("ap-east-1", "Hong Kong", "HK", 22.32, 114.17),
    ("ap-northeast-1", "Tokyo", "JP", 35.68, 139.65),
    ("ap-northeast-2", "Seoul", "KR", 37.57, 126.98),
    ("ap-northeast-3", "Osaka", "JP", 34.69, 135.50),
    ("ap-south-1", "Mumbai", "IN", 19.08, 72.88),
    ("ap-south-2", "Hyderabad", "IN", 17.39, 78.49),
    ("ap-southeast-1", "Singapore", "SG", 1.35, 103.82),
    ("ap-southeast-2", "Sydney", "AU", -33.87, 151.21),
    ("ap-southeast-3", "Jakarta", "ID", -6.21, 106.85),
    ("ap-southeast-4", "Melbourne", "AU", -37.81, 144.96),
    ("asia-east1", "Changhua", "TW", 24.07, 120.56),
    ("asia-east2", "Hong Kong", "HK", 22.32, 114.17),
    ("asia-northeast1", "Tokyo", "JP", 35.68, 139.65),
    ("asia-northeast2", "Osaka", "JP", 34.69, 135.50),
    ("asia-northeast3", "Seoul", "KR", 37.57, 126.98),
    ("asia-south1", "Mumbai", "IN", 19.08, 72.88),
    ("asia-south2", "Delhi", "IN", 28.61, 77.21),
    ("asia-southeast1", "Jurong West", "SG", 1.34, 103.70),
    ("asia-southeast2", "Jakarta", "ID", -6.21, 106.85),
    ("australiaeast", "Sydney", "AU", -33.87, 151.21),
    ("australiasoutheast", "Melbourne", "AU", -37.81, 144.96),
    ("brazilsouth", "Sao Paulo", "BR", -23.55, -46.63),
    ("ca-central-1", "Montreal", "CA", 45.50, -73.57),
    ("ca-west-1", "Calgary", "CA", 51.05, -114.07),
    ("canadacentral", "Toronto", "CA", 43.65, -79.38),
    ("canadaeast", "Quebec City", "CA", 46.81, -71.21),
    ("centralindia", "Pune", "IN", 18.52, 73.86),
    ("centralus", "Iowa", "US", 41.98, -93.62),
    ("eastasia", "Hong Kong", "HK", 22.32, 114.17),
    ("eastus", "Virginia", "US", 37.43, -78.66),
    ("eastus2", "Virginia", "US", 37.43, -78.66),
    ("eu-central-1", "Frankfurt", "DE", 50.11, 8.68),
    ("eu-central-2", "Zurich", "CH", 47.38, 8.54),
    ("eu-north-1", "Stockholm", "SE", 59.33, 18.07),
    ("eu-south-1", "Milan", "IT", 45.46, 9.19),
    ("eu-south-2", "Aragon", "ES", 41.65, -0.88),
    ("eu-west-1", "Dublin", "IE", 53.35, -6.26),
    ("eu-west-2", "London", "GB", 51.51, -0.13),
    ("eu-west-3", "Paris", "FR", 48.86, 2.35),
    ("europe-central2", "Warsaw", "PL", 52.23, 21.01),
    ("europe-north1", "Hamina", "FI", 60.57, 27.20),
    ("europe-southwest1", "Madrid", "ES", 40.42, -3.70),
    ("europe-west1", "St Ghislain", "BE", 50.45, 3.82),
    ("europe-west2", "London", "GB", 51.51, -0.13),
    ("europe-west3", "Frankfurt", "DE", 50.11, 8.68),
    ("europe-west4", "Amsterdam", "NL", 52.37, 4.90),
    ("europe-west6", "Zurich", "CH", 47.38, 8.54),
    ("europe-west8", "Milan", "IT", 45.46, 9.19),
    ("europe-west9", "Paris", "FR", 48.86, 2.35),
    ("francecentral", "Paris", "FR", 48.86, 2.35),
    ("germanywestcentral", "Frankfurt", "DE", 50.11, 8.68),
    ("il-central-1", "Tel Aviv", "IL", 32.08, 34.78),
    ("israelcentral", "Tel Aviv", "IL", 32.08, 34.78),
    ("japaneast", "Tokyo", "JP", 35.68, 139.65),
    ("japanwest", "Osaka", "JP", 34.69, 135.50),
    ("koreacentral", "Seoul", "KR", 37.57, 126.98),
    ("me-central-1", "Dubai", "AE", 25.20, 55.27),
    ("me-south-1", "Manama", "BH", 26.23, 50.59),
    ("northeurope", "Dublin", "IE", 53.35, -6.26),
    ("norwayeast", "Oslo", "NO", 59.91, 10.75),
    ("sa-east-1", "Sao Paulo", "BR", -23.55, -46.63),
    ("southafricanorth", "Johannesburg", "ZA", -26.20, 28.04),
    ("southcentralus", "Texas", "US", 29.42, -98.49),
    ("southeastasia", "Singapore", "SG", 1.35, 103.82),
    ("swedencentral", "Gavle", "SE", 60.67, 17.14),
    ("switzerlandnorth", "Zurich", "CH", 47.38, 8.54),
    ("uaenorth", "Dubai", "AE", 25.20, 55.27),
    ("uksouth", "London", "GB", 51.51, -0.13),
    ("ukwest", "Cardiff", "GB", 51.48, -3.18),
    ("us-central1", "Iowa", "US", 41.26, -95.86),
    ("us-east-1", "Ashburn", "US", 39.04, -77.49),
    ("us-east-2", "Ohio", "US", 40.10, -82.98),
    ("us-east1", "Moncks Corner", "US", 33.20, -80.01),
    ("us-east4", "Ashburn", "US", 39.04, -77.49),
    ("us-east5", "Columbus", "US", 39.96, -83.00),
    ("us-south1", "Dallas", "US", 32.78, -96.80),
    ("us-west-1", "N California", "US", 37.35, -121.96),
    ("us-west-2", "Oregon", "US", 45.84, -119.70),
    ("us-west1", "The Dalles", "US", 45.59, -121.18),
    ("us-west2", "Los Angeles", "US", 34.05, -118.24),
    ("us-west3", "Salt Lake City", "US", 40.76, -111.89),
    ("us-west4", "Las Vegas", "US", 36.17, -115.14),
    ("westeurope", "Amsterdam", "NL", 52.37, 4.90),
    ("westus", "California", "US", 37.77, -122.42),
    ("westus2", "Washington", "US", 47.25, -119.85),
    ("westus3", "Phoenix", "US", 33.45, -112.07),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{DiscoveredConnection, Provider};

    fn connection(key: &str, value: &str) -> DiscoveredConnection {
        DiscoveredConnection {
            id: "probe".to_owned(),
            label: "probe".to_owned(),
            provider: Provider::Aws,
            kind: "ec2-instance".to_owned(),
            metadata: BTreeMap::from([(key.to_owned(), value.to_owned())]),
            commands: Vec::new(),
        }
    }

    #[test]
    fn gazetteer_maps_zones_and_unknown_stays_none() {
        let geo = Gazetteer::default();
        let west = geo
            .locate(&connection("region", "us-west-2"))
            .expect("mapped");
        assert_eq!(west.city, "Oregon");
        assert_eq!(west.source, "estimated-region");

        let zone = geo
            .locate(&connection("zone", "europe-west4-a"))
            .expect("zone strips");
        assert_eq!(zone.city, "Amsterdam");

        assert!(geo.locate(&connection("region", "not-a-region")).is_none());
    }
}
