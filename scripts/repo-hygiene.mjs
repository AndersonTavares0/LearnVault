// Repository hygiene and anti-bloat validation.
//
// Checks tracked files for local artifacts, oversized files, and credentials.

import fs from 'node:fs';
import path from 'node:path';

export const PROHIBITED_PATH_PREFIXES = [
	'.impeccable/',
	'.claude/',
	'.gemini/',
	'.cursor/',
	'.windsurf/',
	'.copilot/',
	'.github/tdd/',
	'docs/design/screenshots/',
	'docs/design/prototypes/',
	'docs/qa/',
	'.pnpm-store/',
	'node_modules/'
];

export const ALLOWED_IMAGE_DIRS = [
	'crates/desktop/icons/',
	'deploy/packages/',
	'testdata/'
];

export const FORBIDDEN_MEDIA_EXTENSIONS = [
	'.png',
	'.jpg',
	'.jpeg',
	'.webp',
	'.avif',
	'.gif',
	'.ico',
	'.bmp',
	'.tiff',
	'.mp4',
	'.mkv',
	'.webm',
	'.avi',
	'.mov'
];

export const MAX_FILE_SIZE_BYTES = 500 * 1024; // 500 KB limit for general source/doc files

const SECRET_PATTERNS = [
	['AWS access key ID', /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/],
	['GitHub token', /\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{82,})\b/],
	['PEM private key header', /-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----/],
	['Slack token', /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/],
	['Google API key', /\bAIza[A-Za-z0-9_-]{35}(?![A-Za-z0-9_-])/],
	['Long sk- key', /\bsk-[A-Za-z0-9_-]{32,}(?![A-Za-z0-9_-])/],
	['JWT', /\beyJ[A-Za-z0-9_-]{7,}\.eyJ[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{16,}(?![A-Za-z0-9_-])/]
];

// Each reviewed exception identifies the fixture path, rule, and exact source line.
// The [P] in the pattern keeps the allowlist declaration from matching the scanner.
const SECRET_FIXTURE_ALLOWLIST = [
	{
		path: 'scripts/repo-hygiene.test.mjs',
		rule: 'PEM private key header',
		line: /^\s*\['PEM private key header', '-----BEGIN OPENSSH [P]RIVATE KEY-----'\],$/
	}
];

function isPrivateLocalPath(file) {
	const name = path.posix.basename(file).toLowerCase();
	if (/^\.env(?:\.|$)/.test(name)) {
		return !/^\.env(?:\..*)?\.example$/.test(name);
	}
	return file.startsWith('deploy/compose/secrets/') || /^id_rsa/.test(name) ||
		/\.(?:pem|key(?:\.pub)?|p12|pfx|jks|keystore|sqlite.*|db|log|local\.md)$/.test(name) ||
		/-(?:wal|shm)$/.test(name);
}

export function checkTrackedFiles(files, { root = process.cwd() } = {}) {
	const violations = [];

	for (const file of files) {
		const normFile = file.replaceAll('\\', '/');
		let violated = false;

		// 1. Prohibited directory prefixes
		for (const prefix of PROHIBITED_PATH_PREFIXES) {
			if (normFile.startsWith(prefix)) {
				violations.push({ file, rule: `File inside prohibited directory '${prefix}'` });
				violated = true;
				break;
			}
		}
		if (violated) continue;

		if (isPrivateLocalPath(normFile)) {
			violations.push({ file, rule: 'Forbidden credential or local file path' });
			continue;
		}

		// 2. docs/design/branding must strictly contain vector SVG and Markdown
		const ext = path.extname(normFile).toLowerCase();
		if (normFile.startsWith('docs/design/branding/') && ext !== '.svg' && ext !== '.md') {
			violations.push({
				file,
				rule: 'docs/design/branding only permits SVG vector assets and Markdown files'
			});
			continue;
		}

		// 3. Forbidden raster and container media outside sanctioned directories
		if (FORBIDDEN_MEDIA_EXTENSIONS.includes(ext)) {
			const allowed = ALLOWED_IMAGE_DIRS.some((dir) => normFile.startsWith(dir));
			if (!allowed) {
				violations.push({
					file,
					rule: `Media file with extension '${ext}' forbidden outside sanctioned directories (${ALLOWED_IMAGE_DIRS.join(', ')})`
				});
				continue;
			}
		}

		// 4. File size ceiling to prevent accidental binary dumps
		const fullPath = path.resolve(root, normFile);
		const stat = fs.lstatSync(fullPath, { throwIfNoEntry: false });
		if (!stat?.isFile()) continue;

		const isLockfile =
			normFile.endsWith('.lock') ||
			normFile.endsWith('.lockb') ||
			normFile.endsWith('pnpm-lock.yaml') ||
			normFile.endsWith('package-lock.json');

		if (stat.size > MAX_FILE_SIZE_BYTES && !isLockfile && !normFile.endsWith('.svg')) {
			violations.push({
				file,
				rule: `File exceeds maximum allowed size of 500 KB (${(stat.size / 1024).toFixed(1)} KB)`
			});
			continue;
		}

		const contents = fs.readFileSync(fullPath);
		if (contents.includes(0)) continue;
		const lines = contents.toString('utf8').split(/\r\n|\n|\r/);
		for (const [index, line] of lines.entries()) {
			for (const [rule, pattern] of SECRET_PATTERNS) {
				if (!pattern.test(line)) continue;
				const allowed = SECRET_FIXTURE_ALLOWLIST.some((fixture) =>
					fixture.path === normFile && fixture.rule === rule && fixture.line.test(line)
				);
				if (!allowed) violations.push({ file, line: index + 1, rule });
			}
		}
	}

	return violations;
}
