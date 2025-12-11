# Procon

_Like Docker, but directly on your machine._

Procon is a project configuration and script manager. It lies in a space similar to Docker, where it allows you to build and run projects. However, Procon runs commands and scripts directly on your machine, instead of relying on virtualization, containers, and images.

## Concept

Below is a (*possibly out of date*) concept of how procon works:

```bash
# Run the `build` script for all projects.
procon run build

# Run `build` then the `start` script.
procon run build start

# Run the `build` scropt for the `blog` project.
procon run build -p blog

```

```yaml
projects:
  airwave-blog:
    dir: projects/blog
    phases:
      clone:
        steps:
          - cwd: ..
            run: git clone <my blog>
      update:
        steps:
          - run: git pull
      build:
        steps:
          - deps:
              - pnpm
            run:
              - pnpm install
              - pnpm build
      deploy:
        steps:
          - task: nginx.copy
          - task: nginx.reload
tasks:
  nginx.copy:
    args: [src]
    steps:
      - run: cp {{src}} /etc/nginx/conf.d/
  nginx.reload:
    steps:
      - run:
          - sudo nginx -t
          - sudo systemctl reload nginx
```
