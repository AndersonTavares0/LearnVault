import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { readIndexedAttestationManifests, resolveArchitectureImageDigest, verifyReleaseIndexAttestations } from './verify-release-index-attestations.mjs';

const amd64PushIndex = `sha256:${'a'.repeat(64)}`;
const arm64PushIndex = `sha256:${'b'.repeat(64)}`;
const amd64 = `sha256:${'e'.repeat(64)}`;
const arm64 = `sha256:${'f'.repeat(64)}`;
const amd64Attestation = `sha256:${'c'.repeat(64)}`;
const arm64Attestation = `sha256:${'d'.repeat(64)}`;
const provenance = 'https://slsa.dev/provenance/v1';
const sbom = 'https://spdx.dev/Document';

function fixture() {
	const descriptor = (digest, architecture) => ({
		digest,
		platform: { os: 'linux', architecture }
	});
	const attestationDescriptor = (digest, subjectDigest) => ({
		digest,
		platform: { os: 'unknown', architecture: 'unknown' },
		annotations: {
			'vnd.docker.reference.type': 'attestation-manifest',
			'vnd.docker.reference.digest': subjectDigest
		}
	});
	const attestationManifest = () => ({
		mediaType: 'application/vnd.oci.image.manifest.v1+json',
		platform: { os: 'unknown', architecture: 'unknown' },
		layers: [
			{ annotations: { 'in-toto.io/predicate-type': provenance } },
			{ annotations: { 'in-toto.io/predicate-type': sbom } }
		]
	});
	const pushIndex = (digest, architecture) => ({
		schemaVersion: 2,
		mediaType: 'application/vnd.oci.image.index.v1+json',
		manifests: [
			{ digest, platform: { os: 'linux', architecture } },
			{ digest: `sha256:${'1'.repeat(64)}`, platform: { os: 'unknown', architecture: 'unknown' }, annotations: { 'vnd.docker.reference.type': 'attestation-manifest', 'vnd.docker.reference.digest': digest } }
		]
	});
	return {
		index: {
			manifests: [
				descriptor(amd64, 'amd64'),
				descriptor(arm64, 'arm64'),
				attestationDescriptor(amd64Attestation, amd64),
				attestationDescriptor(arm64Attestation, arm64)
			]
		},
		pushIndexes: { amd64: pushIndex(amd64, 'amd64'), arm64: pushIndex(arm64, 'arm64') },
		pushDigests: { amd64: amd64PushIndex, arm64: arm64PushIndex },
		digests: { amd64, arm64 },
		attestations: {
			amd64: { [amd64Attestation]: attestationManifest() },
			arm64: { [arm64Attestation]: attestationManifest() }
		}
	};
}

test('verifies that the index contains this run’s amd64 and arm64 digests and both attestations', () => {
	const data = fixture();
	assert.equal(resolveArchitectureImageDigest(data.pushIndexes.amd64, 'amd64'), amd64);
	assert.equal(resolveArchitectureImageDigest(data.pushIndexes.arm64, 'arm64'), arm64);
	assert.equal(verifyReleaseIndexAttestations(data.index, data.digests, data.attestations), true);
});

test('rejects a stale or substituted architecture digest', () => {
	const data = fixture();
	assert.throws(
		() => verifyReleaseIndexAttestations(data.index, { ...data.digests, amd64: `sha256:${'9'.repeat(64)}` }, data.attestations),
		/Published linux\/amd64 digest differs from the digest built and scanned in this run/
	);
});

test('rejects a per-architecture index without exactly one matching platform image manifest', () => {
	const data = fixture();
	data.pushIndexes.amd64.manifests[0].platform.architecture = 'arm64';
	assert.throws(() => resolveArchitectureImageDigest(data.pushIndexes.amd64, 'amd64'), /Expected exactly one linux\/amd64 image manifest/);
});

test('rejects an architecture whose indexed attestation is missing its SBOM statement', () => {
	const data = fixture();
	data.attestations.arm64[arm64Attestation].layers.pop();
	assert.throws(
		() => verifyReleaseIndexAttestations(data.index, data.digests, data.attestations),
		/arm64 is missing its SBOM statement/
	);
});

test('loads exactly the attestation manifests indexed for an architecture', (t) => {
	const data = fixture();
	const directory = mkdtempSync(join(tmpdir(), 'learnvault-index-attestations-'));
	t.after(() => rmSync(directory, { recursive: true, force: true }));
	const amd64Dir = join(directory, 'amd64');
	mkdirSync(amd64Dir);
	writeFileSync(join(amd64Dir, `${amd64Attestation.replace(':', '-')}.json`), JSON.stringify(data.attestations.amd64[amd64Attestation]));
	assert.deepEqual(readIndexedAttestationManifests(data.index, 'amd64', amd64Dir), data.attestations.amd64);
});
