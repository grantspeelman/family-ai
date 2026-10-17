FROM ubuntu:26.04
RUN apt-get update && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run/egress && chown 65534:65534 /run/egress
COPY .devcontainer/egress/proxy.py /usr/local/lib/egress-proxy.py
COPY .devcontainer/egress/allowed-hosts.txt /etc/egress/allowed-hosts.txt
USER 65534:65534
ENTRYPOINT ["python3", "-I", "/usr/local/lib/egress-proxy.py", "gateway"]
