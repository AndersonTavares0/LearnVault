const expectedJobs = [
  'changes',
  'webArtifacts',
  'web',
  'e2eServer',
  'rustContainer',
  'rust',
  'desktop',
  'rustAudit',
];

const filterableJobs = new Set([
  'e2eServer',
  'rustContainer',
  'rust',
  'desktop',
  'rustAudit',
]);

function isSafeRelativePath(file) {
  if (typeof file !== 'string' || file.length === 0) return false;
  if (file.includes('\\') || file.startsWith('/') || /^[a-z]:/i.test(file)) return false;

  return file.split('/').every((segment) => segment !== '' && segment !== '.' && segment !== '..');
}

export function classifyDocsReleaseOnly(paths) {
  return (
    Array.isArray(paths) &&
    paths.length > 0 &&
    paths.every(
      (file) =>
        isSafeRelativePath(file) &&
        (file.startsWith('docs/') || file.endsWith('.md') || file.startsWith('.github/release-please/')),
    )
  );
}

export function evaluateCiGate({ docsReleaseOnly, results } = {}) {
  const errors = [];

  if (typeof docsReleaseOnly !== 'boolean') {
    errors.push('docsReleaseOnly must be a Boolean');
  }
  if (!results || typeof results !== 'object' || Array.isArray(results)) {
    return { passed: false, errors: [...errors, 'CI gate received invalid job results'] };
  }

  const receivedJobs = Object.keys(results).sort();
  const requiredJobs = [...expectedJobs].sort();
  if (receivedJobs.length !== requiredJobs.length || receivedJobs.some((job, index) => job !== requiredJobs[index])) {
    errors.push('CI gate received missing or unexpected job results');
  }

  for (const job of expectedJobs) {
    const result = results[job];
    if (result === 'success') continue;
    if (result === 'skipped' && docsReleaseOnly && filterableJobs.has(job)) continue;
    errors.push(`${job} result was ${result ?? 'missing'}`);
  }

  return { passed: errors.length === 0, errors };
}