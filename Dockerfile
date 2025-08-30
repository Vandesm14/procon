FROM nixos/nix

RUN nix-channel --update
RUN nix-env -iA nixpkgs.yazi nixpkgs.toybox
RUN nix-shell -p rustup --run "rustup default stable"

WORKDIR /build
COPY Cargo.toml .
COPY Cargo.lock .
COPY src src

RUN nix-shell -p rustup --run "cargo build"

RUN mkdir -p /usr/local/bin
ENV PATH="$PATH:/usr/local/bin"
RUN cp target/debug/procon /usr/local/bin
RUN rm -rf /build

WORKDIR /app
COPY test test

WORKDIR /app/test
