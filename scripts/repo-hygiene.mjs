// Repository hygiene and anti-bloat validation.
//
// Ensures that no AI tool scratch directories, unapproved binary images/media,
// or bloated files are tracked in the repository.

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
	'docs/qa/'
];

export const ALLOWED_IMAGE_DIRS = [
	'crates/desktop/icons/',
	'deploy/packages/'
];

export const MAX_FILE_SIZE_BYTES = 500 * 1024; // 500 KB limit for general source/doc files

export function checkTrackedFiles(files, { root = process.cwd() } = {}) {
	const violations = [];

	for (const file of files) {
		let violated = false;

		// 1. Prohibited directory prefixes
		for (const prefix of PROHIBITED_PATH_PREFIXES) {
			if (file.startsWith(prefix)) {
				violations.push({ file, rule: `File inside prohibited directory '${prefix}'` });
				violated = true;
				break;
			}
		}
		if (violated) continue;

		// 2. docs/design/branding must strictly contain vector SVG and Markdown
		const ext = path.extname(file).toLowerCase();
		if (file.startsWith('docs/design/branding/') && ext !== '.svg' && ext !== '.md') {
			violations.push({
				file,
				rule: 'docs/design/branding only permits SVG vector assets and Markdown files'
			});
			continue;
		}

		// 3. Forbidden raster images outside sanctioned icon directories
		if (['.png', '.jpg', '.jpeg', '.webp', '.avif', '.gif', '.mp4'].includes(ext)) {
			const allowed = ALLOWED_IMAGE_DIRS.some((dir) => file.startsWith(dir));
			if (!allowed) {
				violations.push({
					file,
					rule: `Raster media '${ext}' forbidden outside icon directories (${ALLOWED_IMAGE_DIRS.join(', ')})`
				});
				continue;
			}
		}

		// 4. File size ceiling to prevent accidental binary dumps
		try {
			const fullPath = path.resolve(root, file);
			if (fs.existsSync(fullPath)) {
				const stat = fs.statSync(fullPath);
				if (stat.size > MAX_FILE_SIZE_BYTES && !file.endsWith('.lock') && !file.endsWith('.svg')) {
					violations.push({
						file,
						rule: `File exceeds maximum allowed size of 500 KB (${(stat.size / 1024).toFixed(1)} KB)`
					});
				}
			}
		} catch {
			// Ignore filesystem stat errors for files not present in synthetic test runs
		}
	}

	return violations;
}
