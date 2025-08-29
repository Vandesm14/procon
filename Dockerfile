FROM nixos/nix

RUN nix-channel --update
RUN nix-shell -p rustup --run "rustup default stable"

WORKDIR /app
COPY . .

RUN nix-shell -p rustup --run "cargo build"

CMD ["sh"]
