import assert from "node:assert/strict";
import test from "node:test";

import { evaluateCiGate, evaluateCiGateFromNeeds } from "./ci-gate.mjs";

const docsOnlyResults = {
	changes: "success",
	webArtifacts: "success",
	web: "success",
	e2eServer: "skipped",
	rustContainer: "skipped",
	rust: "skipped",
	desktop: "skipped",
	rustAudit: "skipped",
};

const docsOnlyNeeds = {
	"change-scope": {
		result: "success",
		outputs: { docs_release_only: "true" },
	},
	"web-artifacts": { result: "success" },
	web: { result: "success" },
	"e2e-server": { result: "skipped" },
	"rust-container": { result: "skipped" },
	rust: { result: "skipped" },
	desktop: { result: "skipped" },
	"rust-audit": { result: "skipped" },
};

test("the evaluator rejects workflow cancellation despite legitimate docs skips", () => {
	const result = evaluateCiGate({
		docsReleaseOnly: true,
		results: docsOnlyResults,
		workflowCancelled: true,
	});

	assert.equal(result.passed, false);
});

test("the GitHub needs adapter propagates workflow cancellation", () => {
	const result = evaluateCiGateFromNeeds(docsOnlyNeeds, { workflowCancelled: true });

	assert.equal(result.passed, false);
});
