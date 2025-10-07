## TODO

- [ ] Broadcast commands to each project folder
  - [ ] Commands are key-value and defined on a per-project level.
  - [ ] You might run `procon run start` that would run the start script of each project.
  - [ ] `procon run nginx` might be a global script defined at a higher level in the structure to integrate each project into nginx.

<!-- ## User Stories

- I want to provide a GitHub link and have it clone my repo for me
  - Pull on update
  - Target a branch or commit
- I want to tell it how to build and run my project and have it run it
  - Install system deps, build, run
- I want to configure my project for systemctl in the config and run it
- I want to give it a Nginx site file and have it integrate it
  - Build or run-time modules/scripts
- I want to have it manage a remote server
  - SSH

## Lifecycle

- [ ] Commands
  - [x] `plan` (captures config changes and reports dry run)
  - [x] `apply` (applies config changes)
  - [ ] `update` (updates source)
  - [x] `run-proxy` (handles failures from daemons)
- [x] Config
  - [x] Added
    - [x] `Setup`, `Build`, `Start`
  - [x] Changed
    - [x] `Teardown`, `Setup`, `Build`, `Start`
  - [x] Removed
    - [x] `Stop`, `Teardown`
- [ ] Phase
  - [x] `Setup`
    - [x] Installs source
    - [x] Generates daemon (service)
    - [x] Applies daemon
  - [ ] `Update`
    - [ ] Updates source
  - [ ] `Build`
    - [x] Builds source
    - [x] Generates Nginx config
    - [x] Applies Nginx
  - [x] `Start`
    - [x] Starts daemon (proxied)
  - [x] `Stop`
    - [x] Stops daemon
  - [ ] `Teardown`
    - [ ] Stops daemon
    - [ ] Removes daemon
    - [ ] Removes Nginx
    - [ ] Removes artifacts -->
