FROM nixos/nix

RUN nix-channel --update
RUN nix-shell -p rustup --run "rustup default stable"

WORKDIR /build
COPY . .

RUN nix-shell -p rustup --run "cargo build"

RUN mkdir -p /usr/local/bin
ENV PATH="$PATH:/usr/local/bin"
RUN cp target/debug/procon /usr/local/bin

WORKDIR /app
RUN cp -r /build/test/ .
RUN rm -rf /build

WORKDIR /app/test
