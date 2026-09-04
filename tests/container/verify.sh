#!/usr/bin/env sh
set -eu

result_dir=/tmp/access-atlas-container-test
inventory=$result_dir/connections.json
audit=$result_dir/audit.json

mkdir -p "$result_dir"

/workspace/target/release/access-atlas \
  --discover \
  --audit-connections \
  --discovery-home /opt/access-atlas/home \
  --terraform-root /opt/access-atlas/terraform/payments-infra \
  --connections-cache "$inventory" \
  > "$audit"

assert_contains() {
  expected=$1
  file=$2
  if ! grep -F "$expected" "$file" >/dev/null; then
    printf 'expected %s in %s\n' "$expected" "$file" >&2
    exit 1
  fi
}

assert_absent() {
  unexpected=$1
  file=$2
  if grep -F "$unexpected" "$file" >/dev/null; then
    printf 'did not expect %s in %s\n' "$unexpected" "$file" >&2
    exit 1
  fi
}

assert_contains '"passed": true' "$audit"
assert_contains '"connection_count": 10' "$audit"
assert_contains '"command_count": 100' "$audit"
assert_contains '"issues": []' "$audit"

assert_contains '"id": "kubernetes:context:prod-eu"' "$inventory"
assert_contains '"id": "aws:profile:prod"' "$inventory"
assert_contains '"id": "gcloud:configuration:work"' "$inventory"
assert_contains '"id": "azure:subscription:00000000-0000-0000-0000-000000000001"' "$inventory"
assert_contains '"id": "terraform:workspace:/opt/access-atlas/terraform/payments-infra:production"' "$inventory"
assert_contains '"id": "ssh:host:jump-prod"' "$inventory"
assert_contains '"id": "docker:context:remote-prod"' "$inventory"
assert_contains '"id": "tailscale:peer:db-prod.example.ts.net"' "$inventory"
assert_contains '"id": "cloudflare:ingress:ssh.example.test"' "$inventory"
assert_contains '"id": "cloudflare:ingress:app.example.test"' "$inventory"

assert_contains 'kubectl --context prod-eu --namespace payments' "$inventory"
assert_contains 'aws sts get-caller-identity --profile prod' "$inventory"
assert_contains 'gcloud --configuration work' "$inventory"
assert_contains 'az account show --subscription 00000000-0000-0000-0000-000000000001' "$inventory"
assert_contains 'terraform -chdir=/opt/access-atlas/terraform/payments-infra' "$inventory"
assert_contains 'ssh jump-prod' "$inventory"
assert_contains 'docker --context remote-prod' "$inventory"
assert_contains 'tailscale ssh db-prod.example.ts.net' "$inventory"
assert_contains 'cloudflared access ssh --hostname ssh.example.test' "$inventory"
assert_absent 'identity_file' "$inventory"

printf 'container discovery verified: 9 providers, 10 connections, 100 commands\n'
