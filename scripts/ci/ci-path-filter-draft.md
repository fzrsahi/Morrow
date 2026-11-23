# Draft: pasang path router ke pipeline Woodpecker (#369)

Dokumen ini **bukan perubahan aktif**. File `.woodpecker/*.yaml` adalah zona @aazhrt (AGENTS.md §2), jadi tidak diubah di PR #370. Berikut patch siap pakai yang perlu diterapkan oleh owner.

Router: `scripts/ci/path-router.sh`. Ia membaca `CI_PIPELINE_FILES` dan mencetak `linux`, `macos`, `windows`, `release` (kosong untuk docs-only).

---

## `.woodpecker/01-linux.yaml` — step `test-linux`

Ganti blok fast-bypass lama:

```sh
        if [ "${CI_PIPELINE_EVENT:-}" = "pull_request" ] && [ -n "${CI_PIPELINE_FILES:-}" ]; then
          if ! echo "$CI_PIPELINE_FILES" | grep -Eq 'Cargo\.|rust-toolchain|shared/|crates/|linux/|windows/|scripts/ci/(linux|windows|verify|test-multiplatform)|\.woodpecker'; then
            echo "Notice: No Linux/Rust changes detected in PR. Fast-bypass skipping."
            report success "Skipped — No Linux/Rust changes in this PR"
            exit 0
          fi
        fi
```

dengan:

```sh
        if [ "${CI_PIPELINE_EVENT:-}" = "pull_request" ] && ! scripts/ci/path-router.sh | grep -qx linux; then
          echo "Notice: No Linux paths changed in PR. Skipping."
          report success "Skipped — No Linux paths changed"
          exit 0
        fi
```

## `.woodpecker/01-linux.yaml` — step `check-windows`

Blok yang sama diganti dengan `grep -qx windows`, pesan `No Windows paths changed`, dan context GitHub `Morrow / Windows MSVC Check`.

Perubahan `windows/` sekarang hanya menjalankan step Windows, bukan lagi ikut menjalankan `test-linux`.

## `.woodpecker/02-macos.yaml` — step `test`

Ganti blok fast-bypass lama dengan:

```sh
        if [ "${CI_PIPELINE_EVENT:-}" = "pull_request" ] && ! scripts/ci/path-router.sh | grep -qx macos; then
          echo "Notice: No macOS paths changed in PR. Skipping."
          python3 ./scripts/report-status.py --repo "fzrsahi/Morrow" --context "Morrow / Syntax & Git Hygiene Guard" --state success --description "Skipped — No macOS paths changed" || true
          python3 ./scripts/report-status.py --repo "fzrsahi/Morrow" --context "Morrow / XCTest Core Harness" --state success --description "Skipped — No macOS paths changed" || true
          exit 0
        fi
```

## `.woodpecker/03-release.yaml`

Pipeline ini di-trigger tag / push ke `dev`, bukan perubahan path. Tidak ada yang perlu diganti; router hanya dipakai untuk lane test.

---

## Catatan untuk owner

1. `set -euo pipefail` + pipeline: `path-router.sh` selalu keluar 0, dan `grep -q` aman dipakai dalam `if ! …`.
2. Pipeline tetap dibuat Woodpecker, tetapi step yang tidak relevan keluar lebih awal. Kalau Woodpecker mendukung `when: path:` native, itu lebih hemat (pipeline tidak dibuat sama sekali) — silakan dipilih.
3. Perubahan pada router sendiri, `.woodpecker/*`, atau setup runner ⇒ semua lane test tetap jalan (aturan di router).
4. Setelah patch diterapkan, jalankan `bash scripts/ci/test-path-router.sh` untuk memastikan aturan lane masih sesuai.
