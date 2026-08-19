#!/usr/bin/env node
/**
 * lint-cd-loop-risk.mjs
 *
 * Checks all Angular HTML templates for the dangerous pattern:
 *   <mat-option *ngFor="..." (no trackBy)>
 *
 * When a getter returns a new array on every Angular change detection,
 * *ngFor without trackBy rebuilds all <mat-option> DOM nodes inside a
 * <mat-select>. mat-select's ContentChildren listener triggers another
 * CD cycle → infinite loop → browser freeze.
 *
 * Usage:  node scripts/lint-cd-loop-risk.mjs
 *         (returns exit code 1 if violations found)
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = join(import.meta.dirname, '..', 'src', 'app', 'components');
const violations = [];

function walk(dir) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      walk(full);
    } else if (entry.endsWith('.html')) {
      checkFile(full);
    }
  }
}

function checkFile(filePath) {
  const content = readFileSync(filePath, 'utf8');
  const lines = content.split('\n');

  // Simple state machine: track whether we're inside a <mat-select ...>
  let inMatSelect = false;
  let matSelectDepth = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Track mat-select nesting
    if (/<mat-select[\s>]/.test(line)) {
      inMatSelect = true;
      matSelectDepth++;
    }
    if (/<\/mat-select>/.test(line)) {
      matSelectDepth--;
      if (matSelectDepth <= 0) {
        inMatSelect = false;
        matSelectDepth = 0;
      }
    }

    // Check for *ngFor on mat-option without trackBy
    if (inMatSelect && /mat-option/.test(line) && /\*ngFor/.test(line)) {
      if (!/trackBy\s*:/.test(line)) {
        const rel = relative(process.cwd(), filePath);
        violations.push({
          file: rel,
          line: i + 1,
          content: line.trim(),
        });
      }
    }
  }
}

walk(ROOT);

if (violations.length > 0) {
  console.error('\n❌ CD-loop risk: *ngFor on <mat-option> inside <mat-select> without trackBy\n');
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}`);
    console.error(`    ${v.content}\n`);
  }
  console.error('Fix: add trackBy to the *ngFor directive (e.g. trackBy: trackByIndex).');
  console.error('See: docs/angular-cd-loop-prevention.md\n');
  process.exit(1);
} else {
  console.log('✅ No CD-loop risk patterns found.');
}
