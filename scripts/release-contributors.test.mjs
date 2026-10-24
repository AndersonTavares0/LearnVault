import assert from "node:assert/strict";
import test from "node:test";

import { releaseContributors } from "./release-contributors.mjs";

test("excludes Dependabot author and co-author identities", () => {
	const lines = [
		"Dependabot[bot]\t49699333+dependabot[bot]@users.noreply.github.com",
		"Ava Example\tava@example.com",
		"Dependency updater\tdependabot@example.com",
	];

	assert.deepEqual(releaseContributors(lines), ["Ava Example"]);
});

test("deduplicates human contributors and ignores blank or malformed rows", () => {
	const lines = [
		"Zed Contributor\tzed@example.com",
		"Ava Example\tava@example.com",
		"Zed Contributor\tother@example.com",
		"\t",
		"\n",
	];

	assert.deepEqual(releaseContributors(lines), ["Ava Example", "Zed Contributor"]);
});
