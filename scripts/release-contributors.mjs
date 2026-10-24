import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function releaseContributors(lines) {
	const byNormalizedName = new Map();

	for (const line of lines) {
		const [rawName, rawEmail] = line.split("\t");
		const name = rawName?.trim();
		if (!name || !rawEmail || /dependabot/i.test(`${name} ${rawEmail}`)) continue;

		const key = name.toLocaleLowerCase("en-US");
		if (!byNormalizedName.has(key)) byNormalizedName.set(key, name);
	}

	return [...byNormalizedName.values()].sort((left, right) => left.localeCompare(right, "en"));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const lines = readFileSync(0, "utf8").split(/\r?\n/);
	const contributors = releaseContributors(lines);
	if (contributors.length > 0) process.stdout.write(`${contributors.join("\n")}\n`);
}
