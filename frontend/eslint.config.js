// @ts-check
/**
 * ESLint flat config for the Terrane Angular frontend (Angular 22 + ESLint 10).
 *
 * Premade configs come from the `angular-eslint` umbrella package
 * (`tsRecommended` / `templateRecommended` / `templateAccessibility`).
 */
const eslint = require('@eslint/js');
const tseslint = require('typescript-eslint');
const angular = require('angular-eslint');

module.exports = tseslint.config(
  {
    // TypeScript files (components, services, models, ...).
    files: ['**/*.ts'],
    extends: [
      eslint.configs.recommended,
      ...tseslint.configs.recommended,
      ...angular.configs.tsRecommended,
    ],
    languageOptions: {
      parser: tseslint.parser,
    },
    // Lint inline templates inside @Component({ template: `...` }) as well.
    processor: angular.processInlineTemplates,
    rules: {
      // The codebase intentionally stays on NgModule + constructor injection
      // and legacy template control flow (*ngIf/*ngFor); these stylistic
      // migration rules are disabled to keep that style.
      '@angular-eslint/prefer-standalone': 'off',
      '@angular-eslint/prefer-inject': 'off',
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
    extends: [
      ...angular.configs.templateRecommended,
      ...angular.configs.templateAccessibility,
    ],
    languageOptions: {
      parser: angular.templateParser,
    },
    rules: {
      '@angular-eslint/template/prefer-control-flow': 'off',
    },
  },
  {
    // Never lint generated/build artifacts.
    ignores: ['dist/**', 'node_modules/**', '.angular/**'],
  }
);
