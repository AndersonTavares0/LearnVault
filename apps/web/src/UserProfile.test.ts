import { render } from 'svelte/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from './i18n.svelte';
import UserProfile from './UserProfile.svelte';
import { transport, type User } from './transport';

describe('UserProfile component', () => {
	beforeEach(() => setLocale('en'));
	afterEach(() => setLocale('en'));

	const mockUser: User = {
		userId: 'user-123',
		username: 'ander.silva',
		role: 'admin',
		isAdmin: true,
		mustChangeCredentials: false
	};

	it('renders user details, avatar image and file upload button', () => {
		const { body } = render(UserProfile, { props: { user: mockUser } });
		expect(body).toContain('ander.silva');
		expect(body).toContain('Administrator');
		expect(body).toContain('alt="Avatar of ander.silva"');
		expect(body).toContain('Choose photo');

		setLocale('pt-BR');
		const ptRender = render(UserProfile, { props: { user: mockUser } });
		expect(ptRender.body).toContain('Administrador');
		expect(ptRender.body).toContain('alt="Avatar de ander.silva"');
		expect(ptRender.body).toContain('Escolher foto');
	});

	it('renders initials when no avatar url is available', () => {
		const spy = vi.spyOn(transport, 'userAvatarUrl').mockReturnValue('');
		const { body } = render(UserProfile, { props: { user: mockUser } });
		expect(body).toContain('AS');
		spy.mockRestore();
	});

	it('renders student role correctly for non-admin user', () => {
		const studentUser: User = {
			userId: 'user-456',
			username: 'mariana',
			role: 'user',
			isAdmin: false,
			mustChangeCredentials: false
		};
		const { body } = render(UserProfile, { props: { user: studentUser } });
		expect(body).toContain('mariana');
		expect(body).toContain('Student');

		setLocale('pt-BR');
		const ptRender = render(UserProfile, { props: { user: studentUser } });
		expect(ptRender.body).toContain('Estudante');
	});
});
