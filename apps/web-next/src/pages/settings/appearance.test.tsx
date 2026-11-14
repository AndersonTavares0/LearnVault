import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AppearanceSettings } from '@/pages/settings/appearance';
import { DEFAULT_APPEARANCE, resetAppearance } from '@/lib/appearance';

afterEach(() => {
	cleanup();
	resetAppearance(window.localStorage);
});

const root = document.documentElement;
const group = (name: string) => within(screen.getByRole('group', { name }));
const saved = () => JSON.parse(localStorage.getItem('learnvault-appearance') ?? '{}');
const press = (name: string, option: string) => fireEvent.click(group(name).getByRole('button', { name: option }));

describe('AppearanceSettings', () => {
	it.each([
		['Corners', 'Square', 'radius', 'square'],
		['Corners', 'Round', 'radius', 'round'],
		['Density', 'Compact', 'density', 'compact'],
		['Text size', 'Large', 'textSize', 'large'],
		['Text size', 'Small', 'textSize', 'small'],
		['Font', 'System font', 'font', 'system'],
		['Font', 'Reading serif', 'font', 'serif'],
		['Course content in a lesson', 'On the left', 'sidebar', 'left']
	] as const)('%s → %s applies and saves %s=%s', (legend, option, key, expected) => {
		render(<AppearanceSettings />);
		press(legend, option);
		expect(root.dataset[key]).toBe(expected);
		expect(saved()[key]).toBe(expected);
		expect(group(legend).getByRole('button', { name: option }).getAttribute('aria-pressed')).toBe('true');
	});

	it('marks only the current option of a group as pressed', () => {
		render(<AppearanceSettings />);
		const pressed = group('Density').getAllByRole('button').filter((button) => button.getAttribute('aria-pressed') === 'true');
		expect(pressed.map((button) => button.textContent)).toEqual(['Comfortable']);
	});

	it('switches between light and dark and keeps the accent readable in each', () => {
		render(<AppearanceSettings />);
		press('Theme', 'Dark');
		expect(root.dataset.theme).toBe('dark');
		expect(root.style.getPropertyValue('--brand')).toBe('#E3A857');
		press('Theme', 'Light');
		expect(root.dataset.theme).toBe('light');
		expect(root.style.getPropertyValue('--brand')).toBe('#5C4229');
	});

	it('follows the reward colour choice', () => {
		render(<AppearanceSettings />);
		press('Rewards (streak, XP, badges)', 'Same as accent');
		expect(root.style.getPropertyValue('--reward')).toBe('var(--brand)');
		press('Rewards (streak, XP, badges)', 'Amber');
		expect(root.style.getPropertyValue('--reward')).toBe('#A96F2E');
	});

	it.each([
		['Continue learning row', 'showContinue'],
		['Weekly goal', 'showGoal'],
		['Streak', 'showStreak'],
		['Level and XP', 'showXp'],
		['Badges', 'showBadges']
	] as const)('the %s toggle saves %s', (label, key) => {
		render(<AppearanceSettings />);
		const box = screen.getByRole('checkbox', { name: label }) as HTMLInputElement;
		expect(box.checked).toBe(true);
		fireEvent.click(box);
		expect(saved()[key]).toBe(false);
		fireEvent.click(box);
		expect(saved()[key]).toBe(true);
	});

	it('uses a custom colour only when it is a full #RRGGBB value', () => {
		render(<AppearanceSettings />);
		const field = screen.getByLabelText('Your own color');
		const use = screen.getByRole('button', { name: 'Use this color' }) as HTMLButtonElement;
		fireEvent.change(field, { target: { value: '#12' } });
		expect(use.disabled).toBe(true);
		fireEvent.change(field, { target: { value: '#27ae60' } });
		expect(use.disabled).toBe(false);
		fireEvent.click(use);
		expect(saved().accent).toBe('#27AE60');
		expect(screen.getByText('In use: #27AE60.')).toBeTruthy();
	});

	it('explains when a light custom colour is darkened for white text', () => {
		render(<AppearanceSettings />);
		fireEvent.change(screen.getByLabelText('Your own color'), { target: { value: '#FFEE00' } });
		expect(screen.getByText(/too light for white text/)).toBeTruthy();
	});

	it('resets every choice, the custom field and the saved value', () => {
		render(<AppearanceSettings />);
		press('Density', 'Compact');
		fireEvent.change(screen.getByLabelText('Your own color'), { target: { value: '#27AE60' } });
		fireEvent.click(screen.getByRole('button', { name: 'Use this color' }));
		fireEvent.click(screen.getByRole('button', { name: 'Reset to the default look' }));
		expect(root.dataset.density).toBe(DEFAULT_APPEARANCE.density);
		expect((screen.getByLabelText('Your own color') as HTMLInputElement).value).toBe(DEFAULT_APPEARANCE.accent);
		expect(localStorage.getItem('learnvault-appearance')).toBeNull();
	});
});
