# Procon

_Like Docker, but directly on your machine._

Procon is a project configuration and script manager. It lies in a space similar to Docker, where it allows you to build and run projects. However, Procon is different from Docker as it runs commands and scripts directly on your machine, instead of relying on virtualization and images.

## Concept

```bash
# Your directory structure might look something like this:
# projects/
#   my-portfolio/
#     procon.toml
#   blog/
#     procon.toml
#   a-monorepo/
#     package-a/
#       procon.toml
#     package-b/
#       procon.toml

# Run the `build` script for all projects.
procon run build

# Run `build` then the `start` script.
procon run build start
```

```toml
# Dependencies using Nix.
deps.nix = ["pnpm"]

phases.build = "pnpm build"
phases.start = "pnpm start"
```
