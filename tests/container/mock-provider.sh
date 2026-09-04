#!/usr/bin/env sh
set -eu

provider=${0##*/}
arguments=$*

case "$provider:$arguments" in
  "kubectl:config view -o json")
    printf '%s\n' '{"current-context":"prod-eu","contexts":[{"name":"prod-eu","context":{"cluster":"prod-eu-cluster","namespace":"payments","user":"oidc-user"}}],"clusters":[{"name":"prod-eu-cluster","cluster":{"server":"https://api.prod.example.test"}}]}'
    ;;
  "aws:configure list-profiles")
    printf '%s\n' 'prod'
    ;;
  "gcloud:config configurations list --format=json")
    printf '%s\n' '[{"name":"work","is_active":true,"properties":{"core":{"account":"operator@example.test","project":"demo-project"},"compute":{"region":"europe-west4","zone":"europe-west4-a"}}}]'
    ;;
  "az:account list --output json")
    printf '%s\n' '[{"id":"00000000-0000-0000-0000-000000000001","name":"Production","tenantId":"00000000-0000-0000-0000-000000000002","isDefault":true,"state":"Enabled"}]'
    ;;
  "docker:context ls --format {{json .}}")
    printf '%s\n' '{"Name":"remote-prod","Description":"Production engine","DockerEndpoint":"ssh://docker.example.test","Current":true}'
    ;;
  "tailscale:status --json")
    printf '%s\n' '{"Peer":{"nodekey:peer":{"HostName":"db-prod","DNSName":"db-prod.example.ts.net.","TailscaleIPs":["100.64.0.10"],"Online":true,"OS":"linux"}}}'
    ;;
  *)
    printf 'unexpected mock provider invocation: %s %s\n' "$provider" "$arguments" >&2
    exit 64
    ;;
esac
