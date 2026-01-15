# Hibiki

The Rust transaction service on DeltaDeFi, built with [whisky](https://github.com/sidan_lab/whisky).

## Plutus Scripts Sync

The `src/scripts/plutus.json` file is synced from [deltadefi-scripts](https://github.com/deltadefi-protocol/deltadefi-scripts) repo.

| Environment | Sync mechanism |
|-------------|----------------|
| Local dev | `make run` auto-syncs from matching branch |
| delta-defi-local | `./update-scripts.sh` syncs from `develop` |
| CI (CircleCI/GitHub Actions) | Synced before docker build |

### Local Development

```bash
make run          # Auto-syncs plutus.json, then runs server
make sync-plutus  # Manual sync only
```

The sync matches the current git branch. Falls back to `main` if branch doesn't exist in deltadefi-scripts.
