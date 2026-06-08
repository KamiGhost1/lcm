# LCM development image.
#
# Debian-based on purpose: the v1 MVP targets the Debian family, so this image
# doubles as a realistic environment for actually exercising trust-store
# integration (`update-ca-certificates`, `/usr/local/share/ca-certificates`).
FROM rust:1-bookworm

# Build + runtime deps for lcm-core / lcm CLI and for testing integration.
#   pkg-config, libssl-dev      -> native crypto crates (future PKCS#12)
#   ca-certificates, p11-kit    -> system trust store tooling
#   libnss3-tools               -> certutil/pk12util (browser NSS, future)
#   policykit-1                 -> pkexec (privilege escalation path)
#   openssl                     -> generating test certs during development
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        libssl-dev \
        ca-certificates \
        p11-kit \
        libnss3-tools \
        policykit-1 \
        openssl \
        git \
    && rm -rf /var/lib/apt/lists/*

# Node.js for the React/Vite frontend (lcm-gui).
RUN apt-get update && apt-get install -y --no-install-recommends \
        nodejs \
        npm \
    && rm -rf /var/lib/apt/lists/*

# Tauri v2 system dependencies (WebKitGTK + GTK3 + supporting libs) so the
# `lcm-gui` desktop shell can be built and run inside the container.
RUN apt-get update && apt-get install -y --no-install-recommends \
        libwebkit2gtk-4.1-dev \
        libgtk-3-dev \
        libsoup-3.0-dev \
        libjavascriptcoregtk-4.1-dev \
        librsvg2-dev \
        libxdo-dev \
        libayatana-appindicator3-dev \
        patchelf \
        file \
        wget \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_TERM_COLOR=always
WORKDIR /workspace

CMD ["bash"]
