import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptsDir, '..');

const SPEC_FILE_RE = /\.(?:spec|test)\.(?:[cm]?[jt]s|tsx|jsx)$/;

// Every spec in tests/e2e/ currently targets learnvault-server and runs in CI.
// If a future spec must stay out (for example, a desktop-only or separate-server spec),
// list it here as `'tests/e2e/<name>.spec.ts': '<why it must stay out>'` and mirror the
// exclusion in playwright.config.ts / .github/workflows/ci.yml.
export const EXCLUDED_SPECS = Object.freeze({});

export function collectSpecFiles(dirPath, rootDir = root) {
	const entries = readdirSync(dirPath, { withFileTypes: true });
	const specs = [];

	for (const entry of entries) {
		if (entry.name === 'node_modules') continue;
		const fullPath = path.join(dirPath, entry.name);
		if (entry.isDirectory()) {
			specs.push(...collectSpecFiles(fullPath, rootDir));
		} else if (entry.isFile() && SPEC_FILE_RE.test(entry.name)) {
			specs.push(path.relative(rootDir, fullPath).split(path.sep).join('/'));
		}
	}

	return specs.sort();
}

export function extractJobSection(ciYaml, jobName) {
	const pattern = new RegExp(`^  ${jobName}:\\r?\\n([\\s\\S]*?)(?=^  [a-zA-Z0-9_-]+:|$(?![\\r\\n]))`, 'm');
	const match = ciYaml.match(pattern);
	assert.ok(match, `Could not find job "${jobName}" in .github/workflows/ci.yml`);
	return match[1];
}

export function extractPlaywrightCommand(text) {
	const match = text.match(/pnpm\s+exec\s+playwright\s+test[^\n\r;&|]*/);
	return match ? match[0].trim() : null;
}

export function parsePlaywrightConfig(configSource) {
	const testDirMatch = configSource.match(/\btestDir\s*:\s*['"]([^'"]+)['"]/);
	const testDir = testDirMatch ? testDirMatch[1].replace(/^\.\//, '').replace(/\/+$/, '') : null;
	const hasCustomTestMatch = /\btestMatch\s*:/.test(configSource);
	const testIgnoreMatch = configSource.match(/\btestIgnore\s*:\s*(\[[^\]]*\]|['"][^'"]+['"])/);
	const ignoredPatterns = [];

	if (testIgnoreMatch) {
		for (const item of testIgnoreMatch[1].matchAll(/['"]([^'"]+)['"]/g)) {
			ignoredPatterns.push(item[1].replace(/^\.\//, ''));
		}
	}

	return { testDir, hasCustomTestMatch, ignoredPatterns };
}

export function parsePlaywrightCommand(commandLine) {
	const tokens = commandLine.trim().split(/\s+/);
	const testIndex = tokens.indexOf('test');
	const args = testIndex >= 0 ? tokens.slice(testIndex + 1) : [];
	const explicitSpecs = [];
	const ignoredArgs = [];

	for (let i = 0; i < args.length; i += 1) {
		const token = args[i];
		if (token === '--ignore' && args[i + 1]) {
			ignoredArgs.push(args[i + 1].replace(/^\.\//, ''));
			i += 1;
			continue;
		}
		if (token.startsWith('--ignore=')) {
			ignoredArgs.push(token.slice('--ignore='.length).replace(/^\.\//, ''));
			continue;
		}
		if (token.startsWith('-')) {
			if ((token === '--project' || token === '--grep' || token === '--grep-invert' || token === '--workers' || token === '--retries') && args[i + 1]) {
				i += 1;
			}
			continue;
		}
		explicitSpecs.push(token.replace(/^\.\//, '').replace(/\/+$/, ''));
	}

	const runsWholeE2eDir = explicitSpecs.length === 0 || explicitSpecs.includes('tests/e2e');
	return {
		mode: runsWholeE2eDir ? 'all' : 'explicit',
		explicitSpecs,
		ignoredArgs
	};
}

function matchesIgnore(specPath, pattern) {
	const normalized = pattern.replace(/^\*\*\//, '');
	return specPath === pattern || specPath.endsWith(`/${normalized}`) || path.basename(specPath) === normalized;
}

export function auditE2eSpecCoverage({ specFiles, playwrightConfig, ciCommandLine, excludedSpecs = EXCLUDED_SPECS }) {
	const errors = [];

	if (playwrightConfig.testDir !== 'tests/e2e') {
		errors.push(`playwright.config.ts testDir must be "./tests/e2e", found "${playwrightConfig.testDir}"`);
	}
	if (playwrightConfig.hasCustomTestMatch) {
		errors.push('playwright.config.ts must not override testMatch without auditing spec discovery');
	}
	if (!ciCommandLine) {
		errors.push('Missing "pnpm exec playwright test" invocation');
		return errors;
	}

	const command = parsePlaywrightCommand(ciCommandLine);
	const allIgnores = [...playwrightConfig.ignoredPatterns, ...command.ignoredArgs];

	for (const [excludedPath, reason] of Object.entries(excludedSpecs)) {
		if (typeof reason !== 'string' || reason.trim().length === 0) {
			errors.push(`Excluded spec "${excludedPath}" must explain why it is excluded`);
		}
		if (!specFiles.includes(excludedPath)) {
			errors.push(`Excluded spec "${excludedPath}" does not exist in tests/e2e`);
		}
	}

	for (const spec of specFiles) {
		const isExcluded = Object.hasOwn(excludedSpecs, spec);
		const isIgnored = allIgnores.some((pattern) => matchesIgnore(spec, pattern));
		const isExplicitlyListed = command.explicitSpecs.includes(spec);
		const isRunByCommand = command.mode === 'all' ? !isIgnored : isExplicitlyListed && !isIgnored;

		if (isExcluded && isRunByCommand) {
			errors.push(`Excluded spec "${spec}" is still configured to run in CI`);
		}
		if (!isExcluded && !isRunByCommand) {
			errors.push(`Spec "${spec}" in tests/e2e is neither run by CI nor listed in EXCLUDED_SPECS`);
		}
	}

	return errors;
}

test('every spec file in tests/e2e is run by CI and e2e:server or explicitly excluded', () => {
	const specFiles = collectSpecFiles(path.join(root, 'tests/e2e'));
	assert.ok(specFiles.length >= 5, `expected at least 5 spec files, found ${specFiles.length}`);
	assert.ok(specFiles.includes('tests/e2e/web-next.spec.ts'));
	assert.ok(specFiles.includes('tests/e2e/web-next-text-lesson.spec.ts'));

	const playwrightConfig = parsePlaywrightConfig(readFileSync(path.join(root, 'playwright.config.ts'), 'utf8'));
	const ciYaml = readFileSync(path.join(root, '.github/workflows/ci.yml'), 'utf8');
	const e2eServerJob = extractJobSection(ciYaml, 'e2e-server');
	const ciCommandLine = extractPlaywrightCommand(e2eServerJob);

	assert.deepEqual(
		auditE2eSpecCoverage({ specFiles, playwrightConfig, ciCommandLine, excludedSpecs: EXCLUDED_SPECS }),
		[],
		`CI e2e-server must run every spec in tests/e2e or explicitly exclude it (${ciCommandLine})`
	);

	const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
	const localCommandLine = extractPlaywrightCommand(pkg.scripts?.['e2e:server'] ?? '');
	assert.deepEqual(
		auditE2eSpecCoverage({ specFiles, playwrightConfig, ciCommandLine: localCommandLine, excludedSpecs: EXCLUDED_SPECS }),
		[],
		`package.json e2e:server must run every spec in tests/e2e or explicitly exclude it (${localCommandLine})`
	);
});

test('fails when a spec file in tests/e2e is neither run by CI nor listed as excluded', () => {
	const errors = auditE2eSpecCoverage({
		specFiles: ['tests/e2e/web-next.spec.ts', 'tests/e2e/web-next-about.spec.ts'],
		playwrightConfig: { testDir: 'tests/e2e', hasCustomTestMatch: false, ignoredPatterns: [] },
		ciCommandLine: 'pnpm exec playwright test tests/e2e/web-next.spec.ts',
		excludedSpecs: {}
	});

	assert.equal(errors.length, 1);
	assert.match(errors[0], /tests\/e2e\/web-next-about\.spec\.ts/);
});

test('automatically covers newly added specs when CI uses Playwright directory discovery', () => {
	const errors = auditE2eSpecCoverage({
		specFiles: [
			'tests/e2e/web-next.spec.ts',
			'tests/e2e/web-next-recovery.spec.ts',
			'tests/e2e/web-next-about.spec.ts'
		],
		playwrightConfig: { testDir: 'tests/e2e', hasCustomTestMatch: false, ignoredPatterns: [] },
		ciCommandLine: 'pnpm exec playwright test',
		excludedSpecs: {}
	});

	assert.deepEqual(errors, []);
});

test('validates exclusions and rejects unrecorded testIgnore patterns', () => {
	const ignoredUnlisted = auditE2eSpecCoverage({
		specFiles: ['tests/e2e/web-next.spec.ts', 'tests/e2e/desktop-only.spec.ts'],
		playwrightConfig: { testDir: 'tests/e2e', hasCustomTestMatch: false, ignoredPatterns: ['desktop-only.spec.ts'] },
		ciCommandLine: 'pnpm exec playwright test',
		excludedSpecs: {}
	});
	assert.equal(ignoredUnlisted.length, 1);
	assert.match(ignoredUnlisted[0], /desktop-only\.spec\.ts/);

	const excludedWithReason = auditE2eSpecCoverage({
		specFiles: ['tests/e2e/web-next.spec.ts', 'tests/e2e/desktop-only.spec.ts'],
		playwrightConfig: { testDir: 'tests/e2e', hasCustomTestMatch: false, ignoredPatterns: ['desktop-only.spec.ts'] },
		ciCommandLine: 'pnpm exec playwright test',
		excludedSpecs: { 'tests/e2e/desktop-only.spec.ts': 'Requires the Tauri desktop shell, not learnvault-server.' }
	});
	assert.deepEqual(excludedWithReason, []);

	const missingReason = auditE2eSpecCoverage({
		specFiles: ['tests/e2e/web-next.spec.ts', 'tests/e2e/desktop-only.spec.ts'],
		playwrightConfig: { testDir: 'tests/e2e', hasCustomTestMatch: false, ignoredPatterns: ['desktop-only.spec.ts'] },
		ciCommandLine: 'pnpm exec playwright test',
		excludedSpecs: { 'tests/e2e/desktop-only.spec.ts': '   ' }
	});
	assert.equal(missingReason.length, 1);
	assert.match(missingReason[0], /must explain why/);
});
