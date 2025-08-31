## User Stories

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
  - [ ] `plan` (captures config changes and reports dry run)
  - [ ] `apply` (applies config changes)
  - [ ] `update` (updates source)
  - [ ] `run-proxy` (handles failures from daemons)
- [ ] Config
  - [ ] Added
    - [ ] `Setup`, `Build`, `Start`
  - [ ] Changed
    - [ ] `Teardown`, `Setup`, `Build`, `Start`
  - [ ] Removed
    - [ ] `Stop`, `Teardown`
- [ ] Phase
  - [ ] `Setup`
    - [ ] Installs source
    - [ ] Generates daemon (service)
    - [ ] Applies daemon
    - [ ] Generates Nginx config
    - [ ] Applies Nginx
  - [ ] `Update`
    - [ ] Updates source
  - [ ] `Build`
    - [ ] Builds source
  - [ ] `Start`
    - [ ] Starts daemon (proxied)
  - [ ] `Stop`
    - [ ] Stops daemon
  - [ ] `Teardown`
    - [ ] Stops daemon
    - [ ] Removes daemon
    - [ ] Removes Nginx
    - [ ] Removes artifacts
