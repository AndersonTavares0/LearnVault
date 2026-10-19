import { describe, expect, it } from 'vitest';
import type { UpdateStatus } from './transport';
import { updateMessage, updateOutcome, updateReleaseUrl } from './update';

function status(overrides: Partial<UpdateStatus> = {}): UpdateStatus {
	return { current: '0.3.0', enabled: true, available: false, latest: null, releasedAt: null, url: null, checkedAt: '1', error: null, ...overrides };
}

describe('update presentation', () => {
	it('labels an available release with its version and a release link', () => {
		const value = status({ available: true, latest: '0.4.0', releasedAt: '2026-10-17T12:00:00Z', url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0' });
		expect(updateOutcome(value)).toBe('available');
		expect(updateMessage(value)).toBe('Nova versão disponível: 0.4.0.');
		expect(updateReleaseUrl(value)).toBe('https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0');
	});

	it('reports the current version as up to date without a link', () => {
		const value = status();
		expect(updateOutcome(value)).toBe('current');
		expect(updateMessage(value)).toBe('Você está usando a versão mais recente.');
		expect(updateReleaseUrl(value)).toBeNull();
	});

	it('surfaces the sanitized server error', () => {
		const value = status({ error: 'não foi possível contatar o GitHub' });
		expect(updateOutcome(value)).toBe('error');
		expect(updateMessage(value)).toBe('não foi possível contatar o GitHub');
	});

	it('treats an empty error string as no error', () => {
		const value = status({ error: '' });
		expect(updateOutcome(value)).toBe('current');
		expect(updateMessage(value)).toBe('Você está usando a versão mais recente.');
	});

	it('explains a disabled check in one line and never offers a link', () => {
		const value = status({ enabled: false, available: false });
		expect(updateOutcome(value)).toBe('disabled');
		expect(updateMessage(value)).toBe('A verificação de atualização está desligada neste servidor.');
		expect(updateReleaseUrl(value)).toBeNull();
	});

	it('prefers disabled over a residual error field', () => {
		const value = status({ enabled: false, error: 'algo deu errado' });
		expect(updateOutcome(value)).toBe('disabled');
	});

	it('only offers an https release link', () => {
		expect(updateReleaseUrl(status({ available: true, latest: '0.4.0', url: 'http://example.test/release' }))).toBeNull();
		expect(updateReleaseUrl(status({ available: true, latest: '0.4.0', url: 'ftp://example.test/release' }))).toBeNull();
		expect(updateReleaseUrl(status({ available: true, latest: '0.4.0', url: null }))).toBeNull();
	});

	it('only offers the matching release from the official repository', () => {
		const available = { available: true, latest: '0.4.0' };
		expect(updateReleaseUrl(status({ ...available, url: 'https://example.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0' }))).toBeNull();
		expect(updateReleaseUrl(status({ ...available, url: 'https://github.com/attacker/LearnVault/releases/tag/v0.4.0' }))).toBeNull();
		expect(updateReleaseUrl(status({ ...available, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.5.0' }))).toBeNull();
		expect(updateReleaseUrl(status({ ...available, url: 'https://user@github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0' }))).toBeNull();
		expect(updateReleaseUrl(status({ ...available, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0?redirect=1' }))).toBeNull();
		expect(updateReleaseUrl(status({ ...available, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0' }))).toBe('https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0');
		expect(updateReleaseUrl(status({ ...available, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/0.4.0' }))).toBe('https://github.com/AndersonTavares0/LearnVault/releases/tag/0.4.0');
		expect(updateReleaseUrl(status({ ...available, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0+build.7' }))).toBe('https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0+build.7');
	});
});
