# Access Atlas

[![Access Atlas CI](https://github.com/Idanbot/access-atlas/actions/workflows/ci.yml/badge.svg)](https://github.com/Idanbot/access-atlas/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust)](https://www.rust-lang.org/)

**See every environment you can reach, understand how access is wired, and get the right command without hunting through five CLIs.**

Access Atlas is a local-first terminal control map for infrastructure access. It discovers connection metadata already present in `kubectl`, AWS, gcloud, Azure, Terraform, SSH, Docker, Tailscale, and Cloudflare configuration, then turns it into a searchable geographic inventory with context-aware connect, port-forward, and debug command templates.

It does not execute generated commands. You inspect and copy them explicitly.

![Access Atlas cycling through mocked infrastructure targets, access routes, and themes](docs/assets/access-atlas-demo.gif)

> The recording uses the repository's entirely mocked topology. Its hosts, addresses, project names, and account identifiers are fictional.

## Why this should exist

Infrastructure access is usually fragmented across kube contexts, cloud profiles, SSH aliases, tunnels, Terraform roots, and tribal knowledge. During an incident, migration, or onboarding session, the expensive question is often not *what tool should I use?* but *what can this workstation reach, through which identity and route, and what is the safest useful command?*

Access Atlas makes that access surface visible:

- **Incident response:** find the relevant target and copy a metadata-correct diagnostic command quickly.
- **Onboarding and workstation audits:** see which environments are configured, missing, duplicated, or stale.
- **Context safety:** keep provider, account, project, region, cluster, namespace, and route visible beside every command.
- **Access cleanup:** compare authored topology with locally discovered connections and expose unlocated or orphaned entries.
- **Low-risk exploration:** local discovery is the default; online provider inventory is always an explicit action.

The product is intentionally an **operator cockpit and access inventory**, not a credential vault, remote shell, or cloud resource manager.

## Quick start

Requirements: a recent stable Rust toolchain and a terminal with Unicode and true-color support.

```sh
git clone https://github.com/Idanbot/access-atlas.git
cd access-atlas
cargo run --release
```

The app opens immediately with its embedded demo topology. In parallel, it refreshes local connection metadata and reuses a cache no older than 24 hours.

Run an isolated Docker demo:

```sh
docker build --tag access-atlas:dev .
docker run --rm -it access-atlas:dev
```

Validate the fixture without opening the TUI:

```sh
cargo run -- --validate
```

## What you can do

### Explore the atlas

The Braille-dot globe keeps the selected target in view while a great-circle uplink traces the source-to-target route. The command deck shows target health, provider and location metadata, the active access vector, and a scrollable inspection index. Unlocated discoveries remain explicitly unlocated instead of receiving a guessed city.

The active target uses a compact center beacon and restrained pulse. Target changes use a pullback/coast/lock camera movement; route sampling adapts to angular distance so long paths remain accurate and smooth.

### Find discovered connections

Press `g` to open the grouped connection browser. Filter by provider with `Tab`/`Shift+Tab`, search labels, kinds, and metadata with `/`, then press `Enter` to focus a target.

Discovery sources:

| Provider | Local metadata | Explicit online inventory |
| --- | --- | --- |
| Kubernetes | contexts and namespaces from `kubectl` | — |
| AWS | configured profiles | EC2 instances |
| Google Cloud | gcloud configurations | Compute Engine instances |
| Azure | subscriptions | virtual machines |
| Terraform | selected roots and workspaces | — |
| SSH | configured host aliases | — |
| Docker | contexts | — |
| Tailscale | peer metadata | — |
| Cloudflare | Tunnel ingress entries | — |

Local discovery runs in the background. Press uppercase `R` to request read-only provider inventory APIs, `C` to cancel between provider commands, or `R` again to retry a completed, cancelled, or failed refresh. Successful scans remove vanished entries; a failed provider retains its last known cache entries.

Terraform discovery is restricted to the current project unless you add explicit roots with repeatable `--terraform-root PATH`.

### Use command templates

Each resource gets commands matched to its provider, resource kind, and discovered metadata. The first three high-value actions stay one keystroke away with `Tab` and `Shift+Tab`. Press `Enter` on a discovered connection to open its searchable top-10 command library, then `y` to copy the selected command through terminal OSC52.

Access Atlas only renders and copies templates—it never executes them. Typical categories include:

- connect: SSH, SSM, IAP, Azure Bastion, Kubernetes context switching;
- port-forward: Kubernetes services/pods and provider-specific tunnels where meaningful;
- debug: logs, describe/status calls, serial output, guest-agent checks, and network diagnostics.

When a generic category is irrelevant, the slot is filled with a more useful provider-specific action instead of a misleading placeholder.

## Controls

| Key | Action |
| --- | --- |
| `Left` / `Right` | Previous / next target |
| `Tab` / `Shift+Tab` | Next / previous primary command or browser provider |
| `Up` / `Down` | Move the selected detail, connection, or command row |
| `Enter` | Open/close the selected discovered connection's command library |
| `/` | Search the connection browser or command library |
| `Esc` | Clear search or close the active overlay |
| `y` | Copy the selected command with OSC52; never execute it |
| `g` | Open/close the grouped connection browser |
| `R` | Explicitly query configured remote provider APIs |
| `C` | Cancel refresh after the current provider command returns |
| `Space` | Pause/resume automatic target cycling (paused by default) |
| `t` | Cycle five color themes |
| `+` / `-` | Zoom in/out |
| `h` / `j` / `k` / `l` | Orbit longitude/latitude |
| `r` | Recenter on the focused target |
| `q` | Quit |

## Discovery without the TUI

Run a local-only scan and print the generated inventory:

```sh
cargo run -- --discover
```

Explicitly allow provider API queries:

```sh
cargo run -- --discover --online
```

Audit configured providers and every generated template without executing any generated command:

```sh
cargo run -- --discover --audit-connections
```

Acceptance fails for duplicate IDs, malformed command sets, multiline/control-character commands, unresolved placeholders, or credential-shaped metadata. Missing tools and invalid overrides remain warnings so one provider cannot make the rest unusable.

Useful configuration:

| Option / variable | Purpose |
| --- | --- |
| `--connections-cache PATH` / `ACCESS_ATLAS_CACHE` | Override the generated inventory cache |
| `--discovery-home PATH` / `ACCESS_ATLAS_HOME` | Use an isolated discovery home |
| `--cache-max-age-hours HOURS` | Change cache lifetime; `0` disables cache loading |
| `--terraform-root PATH` | Add an explicit Terraform discovery root |
| `--template-overrides PATH` | Load versioned command-template overrides |

Discovery never mutates the authored topology in `data/demo-topology.json`.

## Template overrides

The default override path is `~/.config/access-atlas/templates.json`. Overrides match `provider` plus `resource_kind` and can replace a built-in command by `id` or insert a new command at a position from 1–10. Metadata placeholders such as `{context}`, `{namespace}`, `{profile}`, and `{hostname}` are validated and shell-quoted before insertion. Invalid entries are ignored individually and the safe built-in remains active.

```sh
cargo run -- --discover \
  --template-overrides docs/template-overrides.example.json
```

See the [complete override example](docs/template-overrides.example.json).

## Safety and privacy model

- Generated commands are never run by Access Atlas.
- Online cloud discovery only happens after `--online` or uppercase `R`.
- Discovery retains operational metadata but omits credentials and SSH identity-file contents.
- The inventory cache is separate from the authored demo topology.
- Template values are validated and shell-quoted before interpolation.
- Copying is explicit and uses OSC52; there is no hidden shell handoff.

Treat the generated cache as operational metadata: it can contain internal hostnames, account IDs, project names, and topology details. Apply normal workstation file permissions and avoid committing it.

## Demo data and map sources

The embedded fixture models GCP VMs, a Raspberry Pi behind Cloudflare Access, Kubernetes clusters, a Tailscale node, an AWS instance, and an Azure VM. All addresses are documentation/private ranges and all identifiers are fake.

Coastlines and country boundaries use public-domain [Natural Earth 50m data](https://github.com/nvkelso/natural-earth-vector), embedded in `data/ne_50m_land.json` and `data/ne_50m_boundaries.json`. The renderer builds high-resolution masks with spatial-grid acceleration and samples them during frames.

## Development and tests

Run the complete local quality gate:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run -- --validate
```

Run formatting, Clippy, tests, isolated mock-provider discovery, and JSON validation in Docker:

```sh
./scripts/docker-check.sh
```

The Docker smoke test starts from an isolated home and mock executables, so it verifies fresh-install behavior and deterministic discovered connections without requiring real provider accounts. The same checks run in GitHub Actions.

## Highest-value next improvements

1. **Provider adapter contract:** move scanners and templates behind a documented plugin interface so new CLIs can be added without changing the core app.
2. **Access preflight:** add explicitly requested, non-mutating checks for CLI availability, authentication expiry, DNS, and route prerequisites—while keeping command execution out of scope.
3. **Resource correlation:** connect the same workload across Terraform state, cloud instances, Kubernetes, SSH, and tunnels, with confidence and provenance for every inferred edge.
4. **Inventory history and redacted sharing:** show additions/removals over time and export a deliberately redacted snapshot for incident handoff or onboarding review.
5. **Production distribution:** publish signed binaries, checksums, release notes, and package-manager installs, then add compatibility testing across Linux and macOS terminals.

Those improvements turn the current visual command catalog into a durable access-intelligence layer without turning it into another credential store or remote-execution system.
