# Spec + design notes for this image live in .devcontainer/container-structure-test.yaml.
# Ubuntu LTS, matching the host OS (Dependabot may propose a newer tag; a human decides). Most tools come from its archive; gh, gitleaks,
# lefthook, actionlint, zizmor and bats-core, which the archive lacks or ships too old, are fetched as pinned downloads below.
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
#   • gosu: the entrypoint runs as root (for dockerd) and drops to vscode with it.
#   • docker.io & co.: for `start.sh --docker` only. dockerd is started by
#     /entrypoint.sh when ENABLE_DOCKER=1, which compose.docker.yml sets alongside
#     privileged mode; otherwise the binaries sit unused.
#   • bubblewrap/socat: what Claude Code's Linux sandbox (/sandbox) runs on. They
#     need unprivileged user namespaces, which some hosts restrict.
RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    build-essential \
    ca-certificates \
    cargo \
    curl \
    docker-buildx \
    docker-compose-v2 \
    docker.io \
    git \
    gosu \
    iptables \
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
    shellcheck \
    socat \
    sqlite3 \
    sudo \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI: authenticates from GH_TOKEN, which compose reads from ~/.config/family-ai/gh.env.
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

# actionlint: workflow checks in CI and on staged workflows before commits.
ARG ACTIONLINT_VERSION=1.7.12
ENV ACTIONLINT_SHA256_AMD64=8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8
ENV ACTIONLINT_SHA256_ARM64=325e971b6ba9bfa504672e29be93c24981eeb1c07576d730e9f7c8805afff0c6
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) TOOL_ARCH=amd64; SHA256="${ACTIONLINT_SHA256_AMD64}" ;; \
      arm64) TOOL_ARCH=arm64; SHA256="${ACTIONLINT_SHA256_ARM64}" ;; \
      *) echo "Unsupported arch: $(dpkg --print-architecture)"; exit 1 ;; \
    esac; \
    curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
      "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/actionlint_${ACTIONLINT_VERSION}_linux_${TOOL_ARCH}.tar.gz" \
      -o /tmp/actionlint.tar.gz; \
    echo "${SHA256}  /tmp/actionlint.tar.gz" | sha256sum -c -; \
    mkdir /tmp/actionlint-src; \
    tar -xzf /tmp/actionlint.tar.gz -C /tmp/actionlint-src actionlint; \
    install -m755 /tmp/actionlint-src/actionlint /usr/local/bin/actionlint; \
    rm -rf /tmp/actionlint.tar.gz /tmp/actionlint-src; \
    actionlint -version

# zizmor: workflow checks in CI and on staged workflows before commits.
ARG ZIZMOR_VERSION=1.31.1
ENV ZIZMOR_SHA256_AMD64=d185f726bdaf92df78273217dfdd24ebcd6b0ef48cf3ce776ff5762b6e138c99
ENV ZIZMOR_SHA256_ARM64=37eebbd5762a44f262d94875431b14a6d2402991e90a685113ea97b8b41be833
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) TOOL_ARCH=x86_64; SHA256="${ZIZMOR_SHA256_AMD64}" ;; \
      arm64) TOOL_ARCH=aarch64; SHA256="${ZIZMOR_SHA256_ARM64}" ;; \
      *) echo "Unsupported arch: $(dpkg --print-architecture)"; exit 1 ;; \
    esac; \
    curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
      "https://github.com/zizmorcore/zizmor/releases/download/v${ZIZMOR_VERSION}/zizmor-${TOOL_ARCH}-unknown-linux-gnu.tar.gz" \
      -o /tmp/zizmor.tar.gz; \
    echo "${SHA256}  /tmp/zizmor.tar.gz" | sha256sum -c -; \
    mkdir /tmp/zizmor-src; \
    tar -xzf /tmp/zizmor.tar.gz -C /tmp/zizmor-src zizmor; \
    install -m755 /tmp/zizmor-src/zizmor /usr/local/bin/zizmor; \
    rm -rf /tmp/zizmor.tar.gz /tmp/zizmor-src; \
    zizmor --version

# bats-core: test runner for bash scripts. Released as a source tarball only (no
# release assets), so this is GitHub's archive of the v${BATS_VERSION} tag. It is
# arch-independent, needs only bash, and install.sh puts it under /usr/local.
# The helper libraries (bats-support, bats-assert) are not installed.
# If GitHub regenerates the archive and the checksum check fails, re-verify the
# embedded commit id (`git get-tar-commit-id`) against the tag and re-pin.
ARG BATS_VERSION=1.14.0
ENV BATS_SHA256=bb537b70b15b732f6d8827dd6578e3d8ce166636ce1f18ea9a074184fcce9177
RUN set -eux; \
    curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors \
      "https://github.com/bats-core/bats-core/archive/refs/tags/v${BATS_VERSION}.tar.gz" \
      -o /tmp/bats.tar.gz; \
    echo "${BATS_SHA256}  /tmp/bats.tar.gz" | sha256sum -c -; \
    mkdir /tmp/bats-src; \
    tar -xzf /tmp/bats.tar.gz -C /tmp/bats-src --strip-components=1; \
    /tmp/bats-src/install.sh /usr/local; \
    rm -rf /tmp/bats.tar.gz /tmp/bats-src; \
    bats --version

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
# `git push` neither prompts nor trusts whatever answers on port 22.
COPY .devcontainer/ssh_known_hosts /etc/ssh/ssh_known_hosts

# Create the `vscode` user with the host's UID/GID so bind-mounted files and the
# forwarded ssh-agent socket are owned by the same user on both sides. The
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
    visudo -cf /etc/sudoers.d/vscode; \
    usermod -aG docker vscode

# Claude's config dir is a named volume (see docker-compose.yml) so the login
# survives rebuilds. Creating it here, vscode-owned, means Docker seeds a fresh
# volume with that ownership instead of root's.
# Codex's config dir (CODEX_HOME) is a volume for the same reason.
ENV CLAUDE_CONFIG_DIR=/home/vscode/.claude
ENV CODEX_HOME=/home/vscode/.codex
RUN mkdir -p /workspace "${CLAUDE_CONFIG_DIR}" "${CODEX_HOME}" && \
    chown vscode:vscode /workspace "${CLAUDE_CONFIG_DIR}" "${CODEX_HOME}"
WORKDIR /workspace

COPY .devcontainer/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
# The entrypoint starts as root (for optional dockerd) and gosu-drops to vscode itself.
# hadolint ignore=DL3002
USER root
ENTRYPOINT ["/entrypoint.sh"]
CMD ["sleep", "infinity"]
