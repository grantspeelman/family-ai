# Spec + design notes for this image live in .devcontainer/container-structure-test.yaml.
# Ubuntu LTS, matching the host OS (Dependabot may propose a newer tag; a human decides). Most tools come from its archive; only gh and
# gitleaks, whose archive versions are too old, are fetched as pinned binaries below.
FROM ubuntu:26.04

# pipefail so `curl | tar` and friends fail loudly on a broken download.
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV DEBIAN_FRONTEND=noninteractive

ARG USER_UID=1000
ARG USER_GID=1000

# From the Ubuntu archive:
#   • nodejs/npm (Node 22): only needed to install and run Claude Code and Codex.
#   • python3: for the ad-hoc scripts Claude writes and runs.
#   • rustc/cargo/rustfmt/rust-clippy (Rust 1.93): the toolchain for the registry and
#     family apps. build-essential gives cargo a linker and C compiler, and
#     sqlite3/libsqlite3-dev/pkg-config cover the family app's SQLite database.
#   • gosu: the entrypoint drops to vscode for the container command.
#   • bubblewrap/socat: what Claude Code's Linux sandbox (/sandbox) runs on. They
#     need unprivileged user namespaces, which some hosts restrict.
RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    build-essential \
    ca-certificates \
    cargo \
    curl \
    git \
    gosu \
    jq \
    less \
    libsqlite3-dev \
    nodejs \
    npm \
    openssh-client \
    pkg-config \
    procps \
    python3 \
    python3-pip \
    python3-venv \
    ripgrep \
    rust-clippy \
    rustc \
    rustfmt \
    socat \
    sqlite3 \
    sudo \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI is available for public, unauthenticated queries. Publish on the host.
ARG GH_VERSION=2.102.0
ENV GH_SHA256_AMD64=bb766f710eef8ede859c18578c72c327597cd4c8a85b06001b1f3843c6019386
ENV GH_SHA256_ARM64=7862c86c72f43df3a2d93ddde6f473285b4e2af61b494849846827e513ef6484
RUN set -eux; \
    ARCH=$(dpkg --print-architecture); \
    case "${ARCH}" in \
      amd64) SHA256="${GH_SHA256_AMD64}" ;; \
      arm64) SHA256="${GH_SHA256_ARM64}" ;; \
      *) echo "Unsupported arch: ${ARCH}"; exit 1 ;; \
    esac; \
    curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
      "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${ARCH}.tar.gz" \
      -o /tmp/gh.tar.gz; \
    echo "${SHA256}  /tmp/gh.tar.gz" | sha256sum -c -; \
    tar -xzf /tmp/gh.tar.gz -C /tmp "gh_${GH_VERSION}_linux_${ARCH}/bin/gh"; \
    install -m755 "/tmp/gh_${GH_VERSION}_linux_${ARCH}/bin/gh" /usr/local/bin/gh; \
    rm -rf /tmp/gh.tar.gz "/tmp/gh_${GH_VERSION}_linux_${ARCH}"; \
    gh --version

# gitleaks: run by the lefthook pre-commit hook so secrets never reach a commit.
ARG GITLEAKS_VERSION=8.30.1
ENV GITLEAKS_SHA256_AMD64=551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb
ENV GITLEAKS_SHA256_ARM64=e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) GITLEAKS_ARCH=x64;   SHA256="${GITLEAKS_SHA256_AMD64}" ;; \
      arm64) GITLEAKS_ARCH=arm64; SHA256="${GITLEAKS_SHA256_ARM64}" ;; \
      *) echo "Unsupported arch: $(dpkg --print-architecture)"; exit 1 ;; \
    esac; \
    curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
      "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_${GITLEAKS_ARCH}.tar.gz" \
      -o /tmp/gitleaks.tar.gz; \
    echo "${SHA256}  /tmp/gitleaks.tar.gz" | sha256sum -c -; \
    tar -xzf /tmp/gitleaks.tar.gz -C /usr/local/bin gitleaks; \
    rm /tmp/gitleaks.tar.gz; \
    gitleaks version

# lefthook: git hook runner, configured by /lefthook.yml.
ARG LEFTHOOK_VERSION=2.1.15
ENV LEFTHOOK_SHA256_AMD64=df981b477c236546435047fb8c8ee1f6f902e17faebb1b07b6bf2165832e8fe3
ENV LEFTHOOK_SHA256_ARM64=f3c881e1b6aa7f0461970173b1d692e0eb026bddfcd051cc87f636bf9a9782e0
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) LEFTHOOK_ARCH=x86_64; SHA256="${LEFTHOOK_SHA256_AMD64}" ;; \
      arm64) LEFTHOOK_ARCH=arm64;  SHA256="${LEFTHOOK_SHA256_ARM64}" ;; \
      *) echo "Unsupported arch: $(dpkg --print-architecture)"; exit 1 ;; \
    esac; \
    curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
      "https://github.com/evilmartians/lefthook/releases/download/v${LEFTHOOK_VERSION}/lefthook_${LEFTHOOK_VERSION}_Linux_${LEFTHOOK_ARCH}" \
      -o /usr/local/bin/lefthook; \
    echo "${SHA256}  /usr/local/bin/lefthook" | sha256sum -c -; \
    chmod +x /usr/local/bin/lefthook; \
    lefthook version

# Claude Code floats to latest on purpose: a rebuild should give you the CLI
# Anthropic currently ships, not one frozen when the image was first built.
# Auto-update is off because the global npm prefix is root-owned; rebuild to upgrade.
# Ubuntu's npm 9 predates the package.json `libc` filter, so it also installs the
# musl (Alpine) build of the CLI alongside the glibc one; drop that dead weight.
# hadolint ignore=DL3016
RUN npm install -g @anthropic-ai/claude-code \
    && rm -rf /usr/local/lib/node_modules/@anthropic-ai/claude-code/node_modules/@anthropic-ai/claude-code-linux-*-musl \
    && npm cache clean --force
ENV DISABLE_AUTOUPDATER=1

# Codex CLI floats to latest for the same reason; rebuild to upgrade.
# hadolint ignore=DL3016
RUN npm install -g @openai/codex \
    && npm cache clean --force

# Pin GitHub's SSH host keys (from https://api.github.com/meta) so the first
# public SSH operations use known host identities.
COPY .devcontainer/ssh_known_hosts /etc/ssh/ssh_known_hosts

# Create the `vscode` user with the host's UID/GID so bind-mounted files
# are owned by the same user on both sides. The
# ubuntu base image ships an `ubuntu` user at 1000:1000, which is renamed rather than duplicated.
RUN set -eux; \
    if [ "${USER_UID}" -eq 0 ] || [ "${USER_GID}" -eq 0 ]; then \
        echo "ERROR: USER_UID and USER_GID must not be 0 (got UID=${USER_UID} GID=${USER_GID})" >&2; \
        exit 1; \
    fi; \
    if getent group "${USER_GID}" > /dev/null; then \
        groupmod -n vscode "$(getent group "${USER_GID}" | cut -d: -f1)"; \
    else \
        groupadd -g "${USER_GID}" vscode; \
    fi; \
    if getent passwd "${USER_UID}" > /dev/null; then \
        existing_user="$(getent passwd "${USER_UID}" | cut -d: -f1)"; \
        [ "${existing_user}" = vscode ] || usermod -l vscode "${existing_user}"; \
        usermod -d /home/vscode -m -s /bin/bash -g "${USER_GID}" vscode; \
    else \
        useradd -l -m -d /home/vscode -s /bin/bash -g "${USER_GID}" -u "${USER_UID}" vscode; \
    fi; \
    # Passwordless sudo for the vscode user.
    printf '%s\n' 'vscode ALL=(ALL) NOPASSWD: ALL' > /etc/sudoers.d/vscode; \
    chmod 0440 /etc/sudoers.d/vscode; \
    visudo -cf /etc/sudoers.d/vscode

# Claude's config dir is a named volume (see docker-compose.yml) so the login
# survives rebuilds. Creating it here, vscode-owned, means Docker seeds a fresh
# volume with that ownership instead of root's.
# Codex's config dir (CODEX_HOME) is a volume for the same reason.
ENV CLAUDE_CONFIG_DIR=/home/vscode/.claude
ENV CODEX_HOME=/home/vscode/.codex
RUN mkdir -p /workspace "${CLAUDE_CONFIG_DIR}" "${CODEX_HOME}" && \
    chown vscode:vscode /workspace "${CLAUDE_CONFIG_DIR}" "${CODEX_HOME}"
WORKDIR /workspace

COPY .devcontainer/egress/proxy.py /usr/local/lib/egress-proxy.py
COPY .devcontainer/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
# The entrypoint starts as root and gosu-drops to vscode itself.
# hadolint ignore=DL3002
USER root
ENTRYPOINT ["/entrypoint.sh"]
CMD ["sleep", "infinity"]
