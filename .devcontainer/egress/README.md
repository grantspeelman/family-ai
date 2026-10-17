# Runtime network policy

The application container has `network_mode: none`: only loopback is available.
Both terminal and VS Code entrypoints use the same Compose definition. Local
HTTP servers, test databases and OAuth callback listeners can use loopback inside
the application container. Host/LAN services and sibling containers are not
reachable. Use the CLI's displayed login URL/code in a host browser; Codex's device
login avoids needing a host browser to reach the container's callback listener.
VS Code port forwarding is an explicit host-side capability; do not use it to
introduce a reverse network tunnel. Install editor extensions from the host or a
reviewed VSIX when their download service is outside this policy.

A separate, unprivileged, read-only gateway container owns the destination policy.
Its only shared resource is a Unix socket volume, mounted read-only in the app.
The app's loopback relay adapts `HTTPS_PROXY` clients to that socket. Removing proxy
environment variables, killing or replacing the relay, or gaining root within the
app cannot create a network interface or edit the running gateway's baked policy.
The agent has no gateway process namespace or Docker control socket. Startup
requires a healthy gateway and fails if the Unix socket is unavailable. After a
gateway failure, existing app sessions lose network access; they do not gain a
fallback route. Restart the app after restarting/rebuilding the gateway.

## Destinations

Only exact names in `allowed-hosts.txt` on TCP port 443 are accepted. No wildcards,
plain HTTP forwarding, direct IP authorities, userinfo, alternate ports, or trailing
dots are accepted. Each CONNECT is resolved by the gateway; every answer must be
public. Mixed public/private answers fail closed. The gateway connects to a
validated numeric address without a second lookup. IPv6 is limited to native
global unicast, excluding mapped, 6to4 and Teredo addresses. Redirects are checked
by a new CONNECT when they change destination. A redirected request to an
unapproved service fails; the list never expands automatically.

| Exact hosts | Runtime purpose |
| --- | --- |
| `api.anthropic.com`, `claude.ai`, `platform.claude.com` | Claude API and first-party account authentication |
| `api.openai.com`, `chatgpt.com`, `auth.openai.com` | Codex API-key or ChatGPT model access and authentication |
| `index.crates.io`, `static.crates.io` | Cargo sparse index and locked crate downloads |

Sources checked 2026-10-17: [Claude network configuration](https://code.claude.com/docs/en/network-config),
[Codex authentication](https://developers.openai.com/codex/auth),
[Codex model endpoints](https://openai.com/index/unrolling-the-codex-agent-loop/),
[Cargo registry index](https://doc.rust-lang.org/cargo/reference/registry-index.html)
and the [crates.io index configuration](https://index.crates.io/config.json).
Optional analytics, telemetry, marketplace/plugin downloads, remote MCP services,
web browsing, Git hosting and alternative model providers are outside this policy.
New required hosts need a reviewed policy change and a gateway image rebuild from
a trusted host. Provider changes may require updating this list; do not temporarily
restore unrestricted networking to troubleshoot.

Image builds run on the trusted host/CI builder and use their own network to fetch
Ubuntu packages, npm CLI packages and checksummed release binaries. These are not
runtime permissions. There is no application package-manager stack yet; when one
is introduced, add only its required exact runtime registry hosts with tests.

## Limits and validation

The gateway does not terminate TLS, inspect SNI/HTTP operations, or log requests.
Clients retain ordinary certificate validation. An approved endpoint can accept
uploads and opaque tunnels reach approved resolved IPs (including other TLS SNI/HTTP
hosts on shared CDN addresses); destination filtering
cannot authorize individual requests or prevent all exfiltration. Credentials
still need their own scope and storage controls. Idle tunnels close after five
minutes; clients can reconnect. Gateway policy is immutable within a running
session, but host operators must review source before rebuilding it. Separating
trusted launch files from agent-writable checkouts is covered by plan 05.

`python3 -I .devcontainer/egress/test_proxy.py` uses local Unix/socketpair fixtures
and mocked DNS: allowed CONNECT, denied destinations and malformed requests,
private/mixed/transition DNS answers, pinned resolution and missing socket checks.
`.devcontainer/smoke-test.sh` additionally runs on a Docker host: no external
interfaces or routes, failed IPv4/IPv6 documentation-address connections, resolver
failure, loopback operation, immutable socket mount, an allowed HTTPS request,
a denied destination, a locked Cargo fetch, and failure after stopping the gateway.
The network denial tests do not probe real host, LAN or metadata services: absence
of any external interface/route makes those destinations unreachable as well.

Authenticated acceptance is separate from secret-free CI. From a freshly rebuilt
terminal session, log in to Claude and Codex, send a minimal prompt to each, and
repeat from VS Code. Verify device-code/login flow and streaming responses; these
checks need user accounts and have not been run by the automated tests. Do not
mark that plan acceptance item complete based on a CLI version or unauthenticated
HTTP response.
