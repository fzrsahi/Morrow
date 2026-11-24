// #239 AC 4 — the shared resource, tested rather than trusted.
//
// Three things could go wrong quietly: the generated file drifts from the JSON
// (the gate), a language loses a key macOS has (caught here in CI, since nothing
// runs `swift test`), and the fallback chain differs from Swift's.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { LANGUAGES, resolveLanguage, translate, translator } from './strings';
import { SETTINGS_NAV, pageLabel } from './settings';
import { STRINGS } from './strings.generated';

const root = join(import.meta.dir, '../../../contract/strings');

function json(language: string): Record<string, string> {
  return JSON.parse(readFileSync(join(root, `${language}.json`), 'utf8'));
}

describe('the resource macOS and this app both read', () => {
  test('every language in the resource is emitted and nothing else is', () => {
    expect(Object.keys(STRINGS).sort()).toEqual([...LANGUAGES].sort());
  });

  test('the generated file matches the JSON key for key', () => {
    // The same comparison `strings-codegen.sh --check` makes, so a stray edit to
    // the generated file fails `bun test` and not only the shell gate.
    for (const language of LANGUAGES) {
      const source = json(language);
      expect(STRINGS[language], language).toEqual(source);
      expect(Object.keys(STRINGS[language]).length, language).toBeGreaterThan(500);
    }
  });

  test('English carries every key the other languages are allowed to miss', () => {
    // macOS falls back to `en`; a key missing there would render as its own name
    // in both apps.
    const base = Object.keys(STRINGS.en);
    expect(base.length).toBe(568);
    for (const language of LANGUAGES) {
      const missing = base.filter((key) => !(key in STRINGS[language]));
      // Only the two Chinese tables may be short, and only by the four keys they
      // have always been missing — recorded, not glossed over.
      expect(missing.length, `${language} is missing ${missing.slice(0, 3).join(', ')}`).toBeLessThan(5);
    }
  });

  test('the Settings pages name themselves in the shared file, not in the app', () => {
    // AC 4's proof that the file is *used*: the sidebar's labels come from it.
    for (const page of SETTINGS_NAV.flatMap((group) => group.items)) {
      const shared = page.id in STRINGS.en;
      // `capabilities` is port-only: macOS has no such table, so it keeps its
      // literal rather than inventing a key the other side would not carry.
      expect(shared, page.id).toBe(page.id !== 'capabilities');
    }
    expect(STRINGS.en.shortcuts).toBe('Shortcuts');
    expect(STRINGS.de.shortcuts).toBe('Kurzbefehle');
    expect(STRINGS.zh.general).toBe('通用');
  });
});

describe('the fallback chain matches Swift', () => {
  test('a language that has the key answers in that language', () => {
    expect(translate('sound', 'de')).not.toBe(translate('sound', 'en'));
  });

  test('a key the language is missing falls back to English', () => {
    const missing = Object.keys(STRINGS.en).find((key) => !(key in STRINGS.zh));
    expect(missing, 'the Chinese table is meant to be short somewhere').toBeTruthy();
    expect(translate(missing!, 'zh')).toBe(STRINGS.en[missing!]);
  });

  test('a key nothing carries renders as itself, not as a blank', () => {
    expect(translate('no_such_key_anywhere')).toBe('no_such_key_anywhere');
  });

  test('a translator bound to a language stays bound', () => {
    const de = translator('de');
    expect(de('shortcuts')).toBe(STRINGS.de.shortcuts);
  });
});

describe('the preference becomes a language the resource has', () => {
  test('a stored choice wins over the system preference', () => {
    expect(resolveLanguage('ja', 'de')).toBe('de');
  });

  test('the system preference is narrowed the way Swift narrows it', () => {
    expect(resolveLanguage('zh-TW', 'system')).toBe('zh-Hant');
    expect(resolveLanguage('zh-HK', 'system')).toBe('zh-Hant');
    expect(resolveLanguage('zh-Hans', 'system')).toBe('zh');
    expect(resolveLanguage('zh-CN', 'system')).toBe('zh');
    expect(resolveLanguage('de-AT', 'system')).toBe('de');
    expect(resolveLanguage('tr', 'system')).toBe('tr');
    expect(resolveLanguage('ja-JP', 'system')).toBe('ja');
  });

  test('an unknown preference lands on English rather than a missing table', () => {
    expect(resolveLanguage('fr-FR', 'system')).toBe('en');
    expect(resolveLanguage('', 'system')).toBe('en');
    expect(resolveLanguage('de', 'kl')).toBe('de');
  });
});

describe('the sidebar draws its labels from the shared resource', () => {
  const items = SETTINGS_NAV.flatMap((group) => group.items);

  test('a page with a shared key is labelled from the resource', () => {
    const shortcuts = items.find((item) => item.id === 'shortcuts')!;
    expect(pageLabel(shortcuts, 'en')).toBe(STRINGS.en.shortcuts);
    expect(pageLabel(shortcuts, 'de')).toBe(STRINGS.de.shortcuts);
    expect(pageLabel(shortcuts, 'zh')).toBe(STRINGS.zh.shortcuts);
  });

  test('the one port-only page keeps its literal instead of rendering its id', () => {
    const capabilities = items.find((item) => item.id === 'capabilities')!;
    expect(pageLabel(capabilities, 'en')).toBe('Capabilities');
    expect(pageLabel(capabilities, 'de')).toBe('Capabilities');
  });

  test('a language with no table at all still draws a label', () => {
    // `translate` falls back to English; the sidebar must never render `undefined`.
    for (const item of items) {
      expect(pageLabel(item, 'kl'), item.id).toBeTruthy();
      expect(pageLabel(item, 'kl'), item.id).not.toBe(item.id);
    }
  });
});
