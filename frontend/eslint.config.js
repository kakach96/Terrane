// @ts-check
/**
 * ESLint flat config for the Terrane Angular frontend (Angular 17 + ESLint 8.57).
 *
 * @angular-eslint v17 only ships eslintrc-style shared configs (the flat
 * `tsRecommended` / `templateRecommended` API was introduced in v18), so the
 * shared rules are applied manually from the plugin objects below.
 */
const eslint = require('@eslint/js');
const tseslint = require('typescript-eslint');
const angular = require('@angular-eslint/eslint-plugin');
const angularTemplate = require('@angular-eslint/eslint-plugin-template');
const angularTemplateParser = require('@angular-eslint/template-parser');

module.exports = tseslint.config(
  {
    // TypeScript files (components, services, models, ...).
    files: ['**/*.ts'],
    extends: [
      eslint.configs.recommended,
      ...tseslint.configs.recommended,
    ],
    plugins: {
      '@angular-eslint': angular,
    },
    languageOptions: {
      parser: tseslint.parser,
    },
    // Lint inline templates inside @Component({ template: `...` }) as well.
    processor: angularTemplate.processors['extract-inline-html'],
    rules: {
      // Shared recommended rules from @angular-eslint (eslintrc-style config).
      ...angular.configs.recommended.rules,
      '@angular-eslint/directive-selector': [
        'error',
        { type: 'attribute', prefix: 'app', style: 'camelCase' },
      ],
      '@angular-eslint/component-selector': [
        'error',
        { type: 'element', prefix: 'app', style: 'kebab-case' },
      ],
    },
  },
  {
    // HTML template files (external templates + inline templates extracted
    // by the processor above).
    files: ['**/*.html'],
    plugins: {
      '@angular-eslint/template': angularTemplate,
    },
    languageOptions: {
      parser: angularTemplateParser,
    },
    rules: {
      ...angularTemplate.configs.recommended.rules,
      ...angularTemplate.configs.accessibility.rules,
    },
  },
  {
    // Never lint generated/build artifacts.
    ignores: ['dist/**', 'node_modules/**', '.angular/**'],
  }
);
