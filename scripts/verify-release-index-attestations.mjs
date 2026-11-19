#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const DIGEST = /^sha256:[0-9a-f]{64}$/;
const PROVENANCE_TYPES = new Set([
	'https://slsa.dev/provenance/v0.2',
	'https://slsa.dev/provenance/v1'
]);
const SBOM_TYPES = new Set([
	'https://spdx.dev/Document',
	'https://cyclonedx.org/bom'
]);

function readJson(path) {
	return JSON.parse(readFileSync(path, 'utf8'));
}

export function verifyReleaseIndexAttestations(index, expectedDigests, attestationManifests) {
	for (const architecture of ['amd64', 'arm64']) {
		const expectedDigest = expectedDigests[architecture];
		assert.match(expectedDigest ?? '', DIGEST, `Missing valid ${architecture} build digest`);
		const platformManifests = (index.manifests ?? []).filter((manifest) =>
			manifest.platform?.os === 'linux' && manifest.platform?.architecture === architecture);
		assert.equal(platformManifests.length, 1, `Expected exactly one linux/${architecture} manifest in the published index`);
		assert.equal(platformManifests[0].digest, expectedDigest, `Published linux/${architecture} digest differs from the digest built and scanned in this run`);

		const descriptors = (index.manifests ?? []).filter((manifest) =>
			manifest.annotations?.['vnd.docker.reference.type'] === 'attestation-manifest'
			&& manifest.annotations?.['vnd.docker.reference.digest'] === expectedDigest);
		assert.ok(descriptors.length > 0, `No build attestations in the index refer to linux/${architecture} digest ${expectedDigest}`);

		const supplied = attestationManifests[architecture] ?? {};
		const expectedAttestationDigests = descriptors.map((descriptor) => descriptor.digest).sort();
		assert.deepEqual(Object.keys(supplied).sort(), expectedAttestationDigests, `Could not retrieve every indexed linux/${architecture} attestation manifest`);

		const predicateTypes = new Set();
		for (const [digest, manifest] of Object.entries(supplied)) {
			assert.match(digest, DIGEST, `Invalid ${architecture} attestation digest`);
			for (const layer of manifest.layers ?? []) {
				const predicate = layer.annotations?.['in-toto.io/predicate-type'];
				if (predicate) predicateTypes.add(predicate);
			}
		}
		assert.ok([...PROVENANCE_TYPES].some((type) => predicateTypes.has(type)), `linux/${architecture} is missing its SLSA provenance statement`);
		assert.ok([...SBOM_TYPES].some((type) => predicateTypes.has(type)), `linux/${architecture} is missing its SBOM statement`);
	}
	return true;
}

export function readIndexedAttestationManifests(index, architecture, directory) {
	if (architecture !== 'amd64' && architecture !== 'arm64') throw new Error(`Unsupported architecture: ${architecture}`);
	const platform = (index.manifests ?? []).find((manifest) =>
		manifest.platform?.os === 'linux' && manifest.platform?.architecture === architecture);
	if (!platform) throw new Error(`Published index has no linux/${architecture} manifest`);
	const descriptors = (index.manifests ?? []).filter((manifest) =>
		manifest.annotations?.['vnd.docker.reference.type'] === 'attestation-manifest'
		&& manifest.annotations?.['vnd.docker.reference.digest'] === platform.digest);
	return Object.fromEntries(descriptors.map((descriptor) => {
		const path = join(directory, `${descriptor.digest.replace(':', '-')}.json`);
		return [descriptor.digest, readJson(path)];
	}));
}

function main(args) {
	if (args.length !== 5) {
		throw new Error('Usage: verify-release-index-attestations.mjs <index.json> <amd64-digest> <arm64-digest> <amd64-attestations-dir> <arm64-attestations-dir>');
	}
	const [indexPath, amd64Digest, arm64Digest, amd64Dir, arm64Dir] = args;
	const index = readJson(resolve(indexPath));
	const attestationManifests = {
		amd64: readIndexedAttestationManifests(index, 'amd64', resolve(amd64Dir)),
		arm64: readIndexedAttestationManifests(index, 'arm64', resolve(arm64Dir))
	};
	verifyReleaseIndexAttestations(index, { amd64: amd64Digest, arm64: arm64Digest }, attestationManifests);
	console.log('Verified published amd64 and arm64 manifests, SLSA provenance, and SBOM attestations.');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	try {
		main(process.argv.slice(2));
	} catch (error) {
		console.error(error.message);
		process.exit(1);
	}
}
