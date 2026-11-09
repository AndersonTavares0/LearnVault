import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import test from 'node:test';

const root = resolve(import.meta.dirname, '..');
const read = path => readFileSync(resolve(root, path), 'utf8');
const forms = {
    bug: { name: 'Bug report', ids: ['version', 'steps', 'expected', 'actual'], label: 'bug' },
    feature: { name: 'Feature request', ids: ['problem', 'proposal', 'alternatives', 'acceptance'], label: 'enhancement' },
    documentation: { name: 'Documentation or typo', ids: ['location', 'change'], label: 'documentation' },
    maintenance: { name: 'Dependencies or CI maintenance', ids: ['area', 'change', 'validation'], label: null },
    planning: { name: 'Planning', ids: ['objective', 'scope', 'out_of_scope', 'acceptance_criteria', 'risks'], label: 'planning' }
};

test('issue forms preserve their required field contracts and private security route', () => {
    for (const [file, expected] of Object.entries(forms)) {
        const text = read(`.github/ISSUE_TEMPLATE/${file}.yml`);
        assert.equal(text.match(/^name: (.+)$/m)?.[1], expected.name, file);
        const ids = [...text.matchAll(/^    id: (\w+)$/gm)].map(match => match[1]);
        assert.deepEqual(ids, expected.ids, file);
        assert.equal(new Set(ids).size, ids.length, `${file}: unique IDs`);
        assert.equal([...text.matchAll(/^      required: true$/gm)].length, ids.length, `${file}: required fields`);
        const labels = [...(text.match(/^labels:\n((?:  - .+\n)+)/m)?.[1] ?? '').matchAll(/^  - (.+)$/gm)].map(match => match[1]);
        assert.deepEqual(labels, expected.label ? [expected.label] : [], `${file}: labels`);
        assert.match(text, /Report suspected vulnerabilities privately through the \[security policy\]\(https:\/\/github\.com\/AndersonTavares0\/LearnVault\/security\/policy\)/, file);
        assert.doesNotMatch(text, /[àáâãéêíóôõúç]/i, `${file}: English project copy`);
    }
    const config = read('.github/ISSUE_TEMPLATE/config.yml');
    assert.match(config, /^blank_issues_enabled: false$/m);
    assert.match(config, /https:\/\/github\.com\/AndersonTavares0\/LearnVault\/security\/policy/);
    assert.match(config, /Report vulnerabilities privately; do not open a public issue\./);
});

test('agent entry points reach one canonical review policy and reviewer permissions remain read-only', () => {
    const policy = '.agents/policies/agent-collaboration.md';
    assert.ok(existsSync(resolve(root, policy)));
    for (const entry of ['AGENTS.md', '.agents/repository.md', '.agents/checklists/pull-request.md']) {
        assert.ok(read(entry).includes(policy), `${entry}: canonical policy pointer`);
    }
    for (const profile of ['code-reviewer', 'test-auditor']) {
        const text = read(`.opencode/agents/${profile}.md`);
        for (const action of ['edit', 'shell']) {
            assert.ok(text.includes(`action: ${action}\n    resource: "*"\n    effect: deny`), `${profile}: ${action} denied`);
        }
    }
});
