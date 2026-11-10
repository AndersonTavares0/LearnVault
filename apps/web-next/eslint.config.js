import js from '@eslint/js';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
	{ ignores: ['dist', 'dist-desktop'] },
	js.configs.recommended,
	...tseslint.configs.recommended,
	jsxA11y.flatConfigs.recommended,
	reactHooks.configs.flat.recommended,
	{
		languageOptions: { globals: globals.browser },
		rules: {
			'@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
			// docs/design/security.md: user and file content always renders as text.
			'no-restricted-syntax': ['error', { selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']", message: 'Render user and file content as text.' }]
		}
	}
);
