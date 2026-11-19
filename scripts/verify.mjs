// PR checks for the SPA and for Rust in a single command.
//
// Uso: node scripts/verify.mjs [--fast] [--list]
//   --fast  skips the builds and the audit, keeping type-check, tests and lints.
//   --list  prints the commands that would run and exits without running anything.
//
// This script is the target of `pnpm verify`. The alias must exist because on
// Windows `verify` is a built-in cmd.exe command that exits with code 0 without
// checking anything; without the entry in package.json, `pnpm verify` would fall into it and
// report a false success. Spawn failures (missing tool) also become an
// explicit error, never a silent success.
//
// The desktop crate is outside the workspace's default members; use `pnpm dev:tauri`
// or `cargo test -p learnvault-desktop` when the change touches the desktop.

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));

export function buildSteps({ fast = false } = {}) {
	const steps = [
		{ label: 'pnpm run check', command: 'pnpm', args: ['run', 'check'] },
		{ label: 'pnpm run build', command: 'pnpm', args: ['run', 'build'], slow: true },
		{ label: 'pnpm run build:desktop', command: 'pnpm', args: ['run', 'build:desktop'], slow: true },
		{ label: 'pnpm --filter learnvault-web-next lint', command: 'pnpm', args: ['--filter', 'learnvault-web-next', 'lint'] },
		{
			label: 'pnpm --filter learnvault-web-next test',
			command: 'pnpm',
			args: ['--filter', 'learnvault-web-next', 'test'],
			...(fast ? {} : { env: { LEARNVAULT_PREBUILT_OUTPUTS: '1' } })
		},
		{ label: 'pnpm run test:scripts', command: 'pnpm', args: ['run', 'test:scripts'] },
		{ label: 'pnpm audit --audit-level=low', command: 'pnpm', args: ['audit', '--audit-level=low'], slow: true },
		{ label: 'cargo fmt --all -- --check', command: 'cargo', args: ['fmt', '--all', '--', '--check'] },
		{ label: 'cargo clippy --all-targets -- -D warnings', command: 'cargo', args: ['clippy', '--all-targets', '--', '-D', 'warnings'] },
		{ label: 'cargo test', command: 'cargo', args: ['test'] }
	];

	return fast ? steps.filter((step) => !step.slow) : steps;
}

// Runs the steps in sequence and returns the exit code (0 = everything passed).
// Stops at the first error and never returns 0 when a step does not run.
export async function runSteps(steps, {
	cwd = root,
	spawnImpl = spawn,
	log = console.log,
	errorLog = console.error
} = {}) {
	for (const [index, step] of steps.entries()) {
		log(`[${index + 1}/${steps.length}] ${step.label}`);
		const code = await new Promise((resolve) => {
			const child = spawnImpl(step.command, step.args, {
				cwd,
				stdio: 'inherit',
				shell: process.platform === 'win32',
				...(step.env ? { env: { ...process.env, ...step.env } } : {})
			});
			// A missing tool (ENOENT) emits 'error' without 'exit'; without this handler the
			// promise would never resolve and `pnpm verify` would hang instead of failing.
			child.on('error', () => resolve(1));
			child.on('close', (exitCode, signal) => resolve(exitCode ?? (signal ? 1 : 0)));
		});

		if (code !== 0) {
			errorLog(`\nFailed: ${step.label}\n`);
			return 1;
		}
	}

	return 0;
}

const isMain = process.argv[1] !== undefined
	&& path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
	const args = process.argv.slice(2);
	const fast = args.includes('--fast');
	const steps = buildSteps({ fast });

	if (args.includes('--list')) {
		for (const step of steps) {
			console.log(step.label);
		}
		process.exit(0);
	}

	console.log(`LearnVault verification (${fast ? 'fast: no builds and no audit' : 'full'})`);
	console.log('The desktop crate is left out because it needs native dependencies (WebKit/GTK); use the dedicated commands when the change touches it.\n');

	const code = await runSteps(steps);

	if (code === 0) {
		console.log('\nAll checks passed.');
	}
	process.exit(code);
}
