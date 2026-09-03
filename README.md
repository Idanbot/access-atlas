# Access Atlas

Access Atlas is a Rust terminal application for visualizing machine and cloud access paths. It loads the demo fixture immediately, reads a separate generated connection cache, and discovers connection metadata from already-installed local tools. Generated commands are always read-only templates: Access Atlas never executes them and never reads credential material.

The demo renders a Braille-dot globe with shaded sea, land, coast, graticule, and thin international-border colors. Its Orbital Command Deck keeps the globe dominant while a three-part rail separates target health, the selected access vector, and the scrollable inspection index. It locks the active target into view, centers its approximate city location, and zooms to city scale. The selected target is the only city shown as a globe label. Every value in the command rail comes from `data/demo-topology.json`, including provider metadata, location provenance, network data, health, uptime, latency, and access commands.

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
| `t` | Cycle color theme (Orbital Ice, Tactical Radar, Minimal Atlas, Amber CRT, Deep Space) |
| `+` / `-` | Zoom camera in / out |
| `h` / `j` / `k` / `l` | Manual pan / orbit camera in longitude and latitude |
| `r` | Reset camera to focused target |
| `Tab` | Next primary command (Connect, Port-forward, Debug where relevant) |
| `Shift+Tab` | Previous primary command |
| `Left` / `Right` | Previous or next target |
| `Up` / `Down` | Move the selected detail row |
| `Enter` | Open/close the selected discovered connection's top-10 command library |
| `/` | Search the open command library; `Esc` clears the search or closes it |
| `y` | Copy the selected command through terminal OSC52; never execute it |
| `R` | Explicitly query configured remote provider APIs |
| `q` | Exit |

Auto-cycle is paused by default for focused inspection (press `Space` to run). A target change uses a 1.4-second pullback/coast/lock camera move while the great-circle uplink reveals beneath a traveling photon packet and fading tail. Acquisition finishes even when auto-cycle is held, allowing the renderer to become fully idle afterward. Live mode refreshes the settled packet and countdown at a restrained 6 Hz, while camera acquisition uses the faster animation cadence. The active target features a small amber center beacon and a single restrained pulse ring. The uplink adapts its sampling to the source-to-target angular distance so long routes stay smooth without making local hops heavy. The globe displays high-accuracy continent masks (0.25° sampling), dark stippled oceans, an orbital graticule, atmospheric limb glow, and live camera telemetry.

The data hierarchy is:

```text
target -> network type -> access option
```

For example, the GCP target has an `SSH` network type with three options, while the Kubernetes target has a separate `Kubernetes` type using `kubectl` and a `Helm` type using `helm`. Changing the network type changes the binary and the route semantics instead of treating every command as another flat SSH variant.

## Connection discovery

At TUI startup, Access Atlas renders cached connections immediately and refreshes local metadata on a background thread. Press uppercase `R` for an explicit online refresh. Each source reports loaded, unavailable, or failed independently, so a missing or broken CLI does not discard successful providers.

Supported sources are:

- Kubernetes contexts from `kubectl`
- AWS profiles and, during online refresh, EC2 instances
- gcloud configurations and, during online refresh, Compute Engine instances
- Azure subscriptions and, during online refresh, virtual machines
- explicitly selected Terraform roots and workspaces
- SSH config hosts, Docker contexts, Tailscale peers, and Cloudflare Tunnel ingress entries

Terraform discovery is restricted to the current project by default. Add explicit roots with repeatable `--terraform-root PATH`. The scanners retain operational metadata needed by templates but deliberately omit credentials and SSH identity files.

Run a local-only scan without opening the TUI:

```sh
cargo run -- --discover
```

Allow provider API calls during a one-shot scan:

```sh
cargo run -- --discover --online
```

Use `--connections-cache PATH` to override the generated cache and `--discovery-home PATH` to test an isolated home. Defaults can also be set with `ACCESS_ATLAS_CACHE` and `ACCESS_ATLAS_HOME`. The cache is separate from `data/demo-topology.json`; discovery never mutates the authored topology.

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

Run formatting, Clippy, all tests, the isolated multi-provider discovery smoke test, and JSON validation in the Rust Docker image:

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
