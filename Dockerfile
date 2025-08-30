FROM nixos/nix

RUN nix-channel --update
RUN nix-shell -p rustup --run "rustup default stable"

WORKDIR /app
COPY . .

RUN nix-shell -p rustup --run "cargo build"

RUN mkdir -p /usr/local/bin
ENV PATH="$PATH:/usr/local/bin"
RUN cp /app/target/debug/procon /usr/local/bin
RUN rm -rf /app/target

WORKDIR /app/test
