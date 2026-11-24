// #239 AC 4 — one string resource, shared with macOS.
//
// `shared/contract/strings/<lang>.json` is the source of truth, extracted from
// `macos/Sources/Morrow/L10n.swift` by `scripts/ci/strings-codegen.sh` and
// emitted for both readers. `strings.generated.ts` is not edited by hand; the
// gate in `strings.test.ts` fails when it and the JSON disagree.
//
// The lookup mirrors Swift's `L10n.subscript`, because a UI that falls back
// differently on Windows than on macOS is the same bug as two tables: try the
// chosen language, then `en`, then the key itself — so a missing string is
// visible in the UI instead of blank.
import { STRINGS } from './strings.generated';

// Re-exported so a consumer imports `strings` once, and so the tests compare the
// generated tables against the JSON through the same door the app uses.
export { STRINGS };

/** The languages the resource actually carries. `system` resolves to one. */
export const LANGUAGES = ['en', 'zh', 'zh-Hant', 'de', 'ja', 'ko', 'tr'] as const;

export type Language = (typeof LANGUAGES)[number];

export const BASE_LANGUAGE: Language = 'en';

export function isLanguage(value: unknown): value is Language {
  return typeof value === 'string' && (LANGUAGES as readonly string[]).includes(value);
}

/** The language tag for a BCP-47 preference, the way Swift's `effectiveLanguage`
 *  narrows it: `zh-TW` is Traditional, any other `zh` is Simplified. */
export function resolveLanguage(preferred: string, stored: string): Language {
  if (isLanguage(stored)) return stored;
  const tag = preferred.toLowerCase();
  if (tag.startsWith('zh')) {
    return /^zh-(tw|hk|mo)/.test(tag) || tag.startsWith('zh-hant') ? 'zh-Hant' : 'zh';
  }
  for (const language of LANGUAGES) {
    if (language !== 'zh-Hant' && tag.startsWith(language)) return language;
  }
  return BASE_LANGUAGE;
}

/** The string for `key`, or the key itself when no language carries it. */
export function translate(key: string, language: Language = BASE_LANGUAGE): string {
  return STRINGS[language]?.[key] ?? STRINGS[BASE_LANGUAGE]?.[key] ?? key;
}

/** A translator bound to one language, for a component that renders many. */
export function translator(language: Language = BASE_LANGUAGE): (key: string) => string {
  return (key) => translate(key, language);
}
