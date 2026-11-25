import { describe, expect, test } from 'bun:test';
import { RELEASES_REPOSITORY, releaseNotesUrl } from './release-notes';

describe('releaseNotesUrl', () => {
  test('a plain version maps to its tagged release', () => {
    expect(releaseNotesUrl('0.1.0')).toBe(`${RELEASES_REPOSITORY}/releases/tag/v0.1.0`);
  });

  test('a leading v is not doubled', () => {
    expect(releaseNotesUrl('v1.2.3')).toBe(`${RELEASES_REPOSITORY}/releases/tag/v1.2.3`);
    expect(releaseNotesUrl('V1.2.3')).toBe(`${RELEASES_REPOSITORY}/releases/tag/v1.2.3`);
  });

  test('a version with build metadata stays a tag', () => {
    expect(releaseNotesUrl('1.0.0+build.2')).toBe(
      `${RELEASES_REPOSITORY}/releases/tag/v1.0.0+build.2`,
    );
  });

  test('anything that cannot be a tag opens the release list', () => {
    for (const bad of ['', '  ', 'dev build', 'https://example.com', 'beta.1', 'v']) {
      expect(releaseNotesUrl(bad)).toBe(`${RELEASES_REPOSITORY}/releases`);
    }
  });
});
