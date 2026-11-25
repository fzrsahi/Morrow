// #465 — `ReleaseNotesLink.url(forVersion:)` from
// macos/Sources/MorrowCore/AppDistribution.swift. The About page already had a
// release-list link; this gives it the version-specific one macOS opens.

export const RELEASES_REPOSITORY = 'https://github.com/fzrsahi/morrow';

const TAG_CHARS = /^[0-9A-Za-z.\-+]+$/;

/// The tagged release for `version`, or the release list when the version is
/// not something that can be a tag (a local build, a URL, an empty string).
export function releaseNotesUrl(version: string): string {
  let tag = version.trim();
  if (tag.startsWith('v') || tag.startsWith('V')) tag = tag.slice(1);
  if (!tag || !/^[0-9]/.test(tag) || !TAG_CHARS.test(tag)) {
    return `${RELEASES_REPOSITORY}/releases`;
  }
  return `${RELEASES_REPOSITORY}/releases/tag/v${tag}`;
}
