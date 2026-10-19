import assert from 'node:assert/strict'
import test from 'node:test'

import { classifyDocsReleaseOnly, evaluateCiGate } from './ci-gate.mjs'

const successfulResults = {
  changes: 'success',
  webArtifacts: 'success',
  web: 'success',
  e2eServer: 'success',
  rustContainer: 'success',
  rust: 'success',
  desktop: 'success',
  rustAudit: 'success',
}

test('classifies documentation and release metadata changes as skippable', () => {
  assert.equal(
    classifyDocsReleaseOnly([
      'docs/operations/distribution.md',
      '.github/release-please/release-please-manifest.json',
      'docs/releases/CHANGELOG.md',
    ]),
    true,
  )
})

test('fails safe for empty, mixed, unknown, or unsafe changed paths', () => {
  assert.equal(classifyDocsReleaseOnly([]), false)
  assert.equal(classifyDocsReleaseOnly(['docs/guide.md', 'crates/core/src/lib.rs']), false)
  assert.equal(classifyDocsReleaseOnly(['.github/workflows/ci.yml']), false)
  assert.equal(classifyDocsReleaseOnly(['../docs/guide.md']), false)
  assert.equal(classifyDocsReleaseOnly(['/docs/guide.md']), false)
  assert.equal(classifyDocsReleaseOnly(null), false)
})

test('passes when all CI jobs succeed for a code change', () => {
  assert.equal(evaluateCiGate({ docsReleaseOnly: false, results: successfulResults }).passed, true)
})

test('allows filterable jobs to skip for documentation and release changes', () => {
  const results = {
    ...successfulResults,
    e2eServer: 'skipped',
    rustContainer: 'skipped',
    rust: 'skipped',
    desktop: 'skipped',
    rustAudit: 'skipped',
  }

  assert.equal(evaluateCiGate({ docsReleaseOnly: true, results }).passed, true)
})

test('does not allow the artifact producer or SPA checks to skip', () => {
  for (const job of ['changes', 'webArtifacts', 'web']) {
    const result = evaluateCiGate({
      docsReleaseOnly: true,
      results: { ...successfulResults, [job]: 'skipped' },
    })

    assert.equal(result.passed, false, `${job} must not be skipped`)
  }
})

test('fails when a code-change job unexpectedly skips', () => {
  const result = evaluateCiGate({
    docsReleaseOnly: false,
    results: { ...successfulResults, rust: 'skipped' },
  })

  assert.equal(result.passed, false)
})

test('fails on any failed job or failed change detection', () => {
  for (const [job, status] of [
    ['changes', 'failure'],
    ['webArtifacts', 'failure'],
    ['web', 'failure'],
    ['rustContainer', 'failure'],
    ['rust', 'failure'],
  ]) {
    const result = evaluateCiGate({
      docsReleaseOnly: true,
      results: { ...successfulResults, [job]: status },
    })

    assert.equal(result.passed, false, `${job}=${status} must fail the gate`)
  }
})

test('never passes when a job was cancelled, even for documentation-only changes', () => {
  const result = evaluateCiGate({
    docsReleaseOnly: true,
    results: { ...successfulResults, desktop: 'cancelled' },
  })

  assert.equal(result.passed, false)
})

test('fails closed for unknown, missing, or malformed job results', () => {
  assert.equal(
    evaluateCiGate({
      docsReleaseOnly: false,
      results: { ...successfulResults, rust: 'neutral' },
    }).passed,
    false,
  )
  assert.equal(
    evaluateCiGate({
      docsReleaseOnly: false,
      results: { ...successfulResults, desktop: undefined },
    }).passed,
    false,
  )
  assert.equal(evaluateCiGate({ docsReleaseOnly: false, results: null }).passed, false)
})
