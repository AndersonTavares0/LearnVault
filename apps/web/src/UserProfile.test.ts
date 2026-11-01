import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import UserProfile from './UserProfile.svelte';
import { transport, type User } from './transport';

describe('UserProfile component', () => {
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
		expect(body).toContain('Administrador');
		expect(body).toContain('alt="Avatar de ander.silva"');
		expect(body).toContain('Escolher foto');
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
		expect(body).toContain('Estudante');
	});
});
