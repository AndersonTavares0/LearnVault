# Wiki synchronization playbook

`docs/` and `README.md` are canonical. The `Wiki sync` workflow mirrors only mapped pages
after a merge into `main`; it does not run from pull requests and does not block CI or releases.

To change a managed Wiki page, update its mapped repository document and merge it. Use the
workflow's `workflow_dispatch` trigger only to repair a failed or previously disabled sync.
Pages outside the mapping are not managed by automation.
