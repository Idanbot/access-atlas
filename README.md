# Access Atlas

Access Atlas is a Rust terminal demo for visualizing machine and cloud access paths. Version 1 is deliberately read-only and fixture-driven: it does not contact cloud providers, run commands, authenticate, or read credentials.

The demo renders a sparse Braille-dot globe with shaded sea, land, coast, and thin international-border colors. It locks the active target into view, centers its approximate city location, zooms to city scale, and animates a great-circle route only during target transitions. The selected target is the only city shown as a globe label. The right panel is populated from `data/demo-topology.json`, including provider metadata, city-level location provenance, network data, health, uptime, latency, and access commands.

## Run the demo

From this directory:

```sh
cargo run
```

For a reproducible Docker-only run:

```sh
docker build --tag access-atlas:dev .
docker run --rm -it access-atlas:dev
```

The demo also embeds the fixture, so the compiled binary can fall back to the same data when the default JSON path is not available.

Validate the fixture without opening a terminal UI:

```sh
docker run --rm access-atlas:dev --validate
```

## Controls

| Key | Action |
| --- | --- |
| `Space` | Pause / resume automatic target cycling (paused by default) |
| `t` | Cycle color theme (Tactical Radar, Minimal Atlas, Cyber Orbital, Amber CRT, Deep Space) |
| `+` / `-` | Zoom camera in / out |
| `h` / `j` / `k` / `l` | Manual pan / orbit camera in longitude and latitude |
| `r` | Reset camera to focused target |
| `Tab` | Next access option; crosses into the next network type at the end |
| `Shift+Tab` | Previous access option; crosses into the previous network type at the beginning |
| `Left` / `Right` | Previous or next target |
| `Up` / `Down` | Move the selected detail row |
| `Enter` | Reserved; currently does nothing |
| `q` | Exit |

Auto-cycle is paused by default for focused inspection (press `Space` to run). The route is animated along a subtle, thin 1-dot 3D parabolic great-circle arc with a traveling photon packet and fading tail from the local workstation origin to the active target. The active target features a clean circular reticle and expanding concentric radar beacon. The globe displays high-accuracy continent masks (0.5° sampling), clean dark oceans, atmospheric Rayleigh limb glow, world-space locked dithering, and live orbital telemetry badges.

The data hierarchy is:

```text
target -> network type -> access option
```

For example, the GCP target has an `SSH` network type with three options, while the Kubernetes target has a separate `Kubernetes` type using `kubectl` and a `Helm` type using `helm`. Changing the network type changes the binary and the route semantics instead of treating every command as another flat SSH variant.

## Demo data

The fixture includes mocked examples for:

- GCP Compute Engine `e2-micro` in Amsterdam using three IAP/SSH options and serial console access
- GCP Compute Engine `e2-micro` in Ashburn using three IAP/SSH options and serial console access
- A Raspberry Pi 4 in Tel Aviv using Cloudflare Access, tunnel, alias, and LAN SSH options
- Kubernetes API access through `kubectl` plus Helm release inspection in Frankfurt
- Kubernetes API access through `kubectl` plus Helm release inspection in Tokyo
- Tailscale SSH, direct tailnet access, status, and network diagnostics in Berlin
- AWS SSM, API, bastion, and SSH access in Frankfurt
- Azure CLI, Bastion, VM inventory, and guest-agent access in Dublin

The coastline mask and thin country-border overlay use public-domain [Natural Earth 50m data](https://github.com/nvkelso/natural-earth-vector), embedded in `data/ne_50m_land.json` and `data/ne_50m_boundaries.json`. The renderer converts those source geometries into high-resolution 0.25-degree masks (1440×720) with spatial-grid acceleration, then samples the masks during frames. All addresses are documentation or private addresses, and identifiers are fake. The fixture is not an instruction to connect to any target.

## Docker checks

Run formatting, Clippy, unit tests, and JSON validation in the Rust Docker image:

```sh
./scripts/docker-check.sh
```

The same checks run in GitHub Actions. The project uses the latest versions resolved from crates.io when the lockfile is regenerated; `Cargo.lock` is committed for reproducible checks.

## Development quality gate

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run -- --validate
```
