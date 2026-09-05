# Security Policy

Access Atlas is a local-first operator cockpit. It reads connection metadata
already present on the workstation, renders command templates, and copies them
only when you press `y`. It does not execute generated commands.

## Supported versions

Security fixes land on `main`. There are no stable release branches yet.

## Reporting a vulnerability

Please use GitHub private vulnerability reporting:

https://github.com/Idanbot/access-atlas/security/advisories/new

Do not open a public issue for a vulnerability. Private reporting is available
on public repositories; enable it under Settings → Code security after going
public. Until then, create a draft advisory from the Security tab.

Reports of particular interest:

- credentials, tokens, or SSH identity-file contents reaching the inventory cache or TUI
- generated commands being executed
- cache files written with group or world access
- online provider APIs being queried without `--online` or uppercase `R`

## Operational metadata

The inventory cache is owner-read/write only (`0600`) and can still contain
hostnames, account IDs, project names, and kube API servers. Treat it as
sensitive workstation state.
