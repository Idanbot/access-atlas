FROM rust:latest

WORKDIR /workspace
COPY . .

RUN cargo build --locked --release

COPY --chmod=755 tests/container/mock-provider.sh /opt/access-atlas/mock-bin/mock-provider
RUN for provider in kubectl aws gcloud az docker tailscale; do \
      ln -s mock-provider "/opt/access-atlas/mock-bin/$provider"; \
    done

COPY tests/container/fixtures/home/ /opt/access-atlas/home/
COPY tests/container/fixtures/terraform/ /opt/access-atlas/terraform/
COPY --chmod=755 tests/container/verify.sh /usr/local/bin/verify-access-atlas-container

ENV PATH="/opt/access-atlas/mock-bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

ENTRYPOINT ["/usr/local/bin/verify-access-atlas-container"]
