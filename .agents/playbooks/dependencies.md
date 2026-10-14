# Dependency update playbook

1. Rebase or recreate the update from current `main`.
2. Review release notes and peer/runtime compatibility before changing manifests.
3. Run the required CI-equivalent checks and inspect lockfile changes.
4. Keep major upgrades separate unless the compatible ecosystem upgrade is intentionally scoped.
5. Do not force peer dependency resolution or merge a stale Dependabot pull request.
