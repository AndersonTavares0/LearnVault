#!/bin/sh
set -eu

wiki_dir=${1:?usage: sync-wiki.sh <wiki-directory>}

sync_page() {
	source=$1
	destination=$2
	{
		printf '%s\n\n' '<!-- Managed by LearnVault repository. Edit the mapped source, not this Wiki page. -->'
		cat "$source"
	} > "$wiki_dir/$destination.md"
}

sync_page docs/wiki/home.md Home
sync_page docs/wiki/getting-started.md Getting-Started
sync_page docs/architecture/overview.md Architecture
sync_page docs/product/roadmap.md Roadmap
sync_page docs/product/requirements.md Requirements
sync_page docs/governance/overview.md Development-and-Governance
sync_page docs/operations/distribution.md Operations
