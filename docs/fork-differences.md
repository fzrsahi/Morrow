# Apa yang berubah dari CodeIsland

Morrow adalah fork dari [wxtsky/CodeIsland](https://github.com/wxtsky/CodeIsland). Upstream adalah aplikasi SwiftUI/AppKit untuk macOS. Fork ini menambah shell Rust/Tauri untuk Linux dan Windows, mengubah banyak jalur konfigurasi, dan menambah fitur yang tidak disebut upstream.

Daftar ini membandingkan `wxtsky/CodeIsland` `main` (README upstream 396 baris) dengan repo ini. Bukti yang dipakai adalah kode, ADR, dan README, bukan perkiraan. Status "terpasang" berarti kodenya ada di shell atau UI. Status "pustaka saja" berarti modulnya ada tetapi belum dipanggil UI.

## Identitas dan jalur

| Perubahan | Bukti |
|---|---|
| Nama produk CodeIsland → Morrow, repo → `fzrsahi/morrow` | README kita `:8-23`; upstream `:1-15`. Folder fisik repo masih bernama `CodeIsland`, tapi path repo dan artifact sudah `morrow`. |
| Socket IPC `/tmp/codeisland-<uid>.sock` → `/tmp/morrow-<uid>.sock` | README kita `:355` vs upstream `:322`. #78/PR #161 menambah resolusi ke socket legacy saat kirim agar hook lama tidak hilang. |
| Bridge `codeisland-bridge` → `morrow-bridge` | README kita `:355` vs upstream `:322`; `build.sh` menyalin keduanya dengan alias. |
| Delapan jalur hook/plugin diganti nama | README kita `:200-215` vs upstream `:166-181`: Grok, Copilot, Kiro, MiniMax, OpenCode, MiMo, Pi/Oh My Pi, OpenClaw. Kiro juga memakai `kiro --agent morrow`. |
| Homebrew `codeisland` → `morrow`, DMG dan app → `Morrow.*` | README kita `:227-233` vs upstream `:193-199`. |
| Companion path `ios/CodeIslandCompanion` → `ios/MorrowCompanion` | README kita `:257` vs upstream `:212`. |

Daftar tool yang didukung **tidak berubah**. Grid 28 tool di README kita `:137-195` sama dengan upstream `:103-150`. Yang berubah hanya path ikon dan nama file. Tidak ada tool yang benar-benar ditambah atau dihapus dari daftar itu.

## Arsitektur

| Perubahan | Bukti |
|---|---|
| Dua core, bukan satu. macOS tetap Swift; Linux/Windows memakai core Rust. Keduanya hanya berbagi kontrak, bukan kode. | ADR-0001 `:5`, `:34`, `:38-39`; ADR-0003 `:3,10` menolak Opsi E. Biayanya: dua core dijaga bersamaan, port ≈37–40K LOC. |
| Shell Tauri v2 + Svelte 5 menggantikan WPF di Windows dan jadi panel Linux. | ADR-0002 `:5,7,9,15-16`; `shared/desktop/src-tauri/tauri.conf.json`. Keputusan ini didasarkan pada satu shell untuk dua OS, bukan performa. |
| `morrow-core` murni dan dijaga `rust-purity.sh`: tidak boleh bergantung pada `windows`, `libc`, `nix`, `zbus`, `tokio`. | `docs/plan-windows-linux.md:99-105,147,176`; `shared/ci/rust-purity.sh`. Efek samping lewat `Clock`, `CustomSources`, `FileSystem`. |
| Satu tabel capability jadi sumber kebenaran. Baris `Supported` harus punya adapter di `ADAPTERS`, kalau tidak test gagal. | `docs/plan-windows-linux.md:218,226`; `capability.rs`; `tests/capability_and_settings.rs`. #235 menurunkan 11 baris Linux dan 8 Windows ke `Unsupported(reason)`. |
| Kontrak hook dan endpoint dibekukan di `shared/contract/`, termasuk golden fixture dan aturan peer verification. | ADR-0001 `:5`; `docs/plan-windows-linux.md:56-64,118-130`; `shared/contract/hook-edge-protocol.md`. |
| Monorepo: `macos/`, `shared/`, `linux/`, `windows/`. | #86, PR #171 (`9680d4a5`). 712 file dipindah tanpa perubahan perilaku. |
| Panel Linux memakai tight-fit window dan nonactivating, bukan click-through GTK. | `docs/plan-windows-linux.md:268-303`; ADR-0002 `:20-25`. |
| CI lokal wajib sebelum push; gate host tidak dijalankan per PR. | `AGENTS.md:54-68`; `docs/plan-windows-linux.md:349,363,463-465`. |

## Fitur yang ditambahkan di fork

### Terpasang di UI atau shell

- **Pencarian sesi dari proses yang berjalan.** 13 reader: `ClaudeTranscripts`, `OpenCodeStore`, `CodexRollouts`, `ClineStore`, `CursorTranscripts`, `GeminiChats`, `KimiSessions`, `KiroSessions`, `CopilotSessions`, `GrokSessions`, dan tiga `FlatStore` (qoder/codebuddy/droid). Bukti: `morrow-core/src/discovery.rs`, `src-tauri/src/discovery.rs`, `src-tauri/src/feed.rs::scan_stores`. Upstream hanya melihat sesi lewat hook.
- **Backfill saat attach.** Sesi yang sudah berjalan muncul sebelum hook pertama. Bukti: `feed.rs::backfill_with`, `morrow-core/src/session_store.rs`, #261/#339.
- **Riwayat balasan Markdown yang dipangkas jadi satu baris kartu.** `morrow-core/src/markdown_preview.rs::{plain, preview_source, segments, code_summary}`, dipakai `MarkdownReply.svelte`.
- **Pengingat lanjutan.** `morrow-core/src/follow_up.rs`, `src-tauri/src/reminders.rs::ReminderBook`, `lib/reminders.ts`.
- **Doa dan kota offline.** `morrow-core/src/prayer.rs`, `prayer/cities.rs`, `src-tauri/src/prayer.rs`, `PrayerCard.svelte`, `PrayerPage.svelte`. #296/#305/#318.
- **Warna badge dan tema yang mengikuti pengaturan.** `lib/tokens.ts`, `session-cards.ts`, `AppearancePage.svelte`. #391.
- **Tiga tab muzik, visualizer spektrum, dan floating lirik sub-notch dengan empat preset.** README kita `:31-73`; tidak ada di upstream. Preset: `pureText`, `capsule`, `frostedGlass`, `neonHud`.
- **Git graph interaktif dan workspace diff.** README kita `:67-70`; upstream hanya menyebut branch/worktree di kartu.
- **Perbaikan privasi idle dot.** Ikon hijau diganti `chevron.down`/`terminal` agar tidak menyerupai indikator kamera macOS. README kita `:72-73`.

### Pustaka saja, belum dipakai UI

- Push: `morrow-core/src/push.rs`, `push_policy.rs`, `push_transport.rs`, `push_channel.rs`. Tidak ada command `push` di `src-tauri/src/main.rs`. #350/#358/#363.
- Kuota Claude: `morrow-core/src/quota.rs`, `morrow-platform/src/quota.rs`. Kartu kuota yang terlihat masih di sisi Swift (`ClaudeQuotaMonitor.swift`). #297/#306.

Ini bukan fitur yang sudah bisa dipakai di Linux/Windows. Bedakan dari yang "terpasang" di atas.

## Yang dihapus atau tidak lagi ditampilkan

| Item | Bukti |
|---|---|
| Bagian Star history | Upstream `:384-392`. Grafiknya memakai `star-history.dera.page`, yang mengembalikan 404 untuk repo kita. Penghapusannya ada di PR #406. |
| Switch bahasa `English | 简体中文` dan tautan `README.zh-CN.md` | Upstream `:27`; tidak ada di README kita. Catatan: teks Highlight kita masih menyebut 7 bahasa UI. |
| Badge Downloads dan App Store Buddy di baris atas | Upstream `:13,15`; tidak di badge kita. Fitur Buddy sendiri masih didokumentasikan. |
| Tautan "How it works" di nav | Upstream `:24`; bagiannya tetap ada di README kita `:351`. |
| Bagian Contributing | Ditambahkan di fork, README kita `:433`; upstream tidak punya. |

## Kesiapan platform

Linux dan Windows masih **eksperimental**. Kode ter-compile dan test Rust lolos, tetapi gate host belum jalan: E2E-1/E2E-2, L-001…L-012, dan W-001…W-009. Rincian dan perintah build ada di `docs/build-windows-linux.md` (PR #406). Utangnya tercatat di #399, #400, #401, #402.

Jangan membaca keberadaan modul Rust sebagai "sudah setara dengan macOS". `morrow-watch` misalnya baru ter-compile untuk Linux/Windows dan belum pernah memantau direktori nyata di OS itu.

## Cara memverifikasi daftar ini

```sh
grep -n '^## ' README.md
grep -n '^## ' docs/plan-windows-linux.md
cat docs/adr/0001-two-cores-one-contract.md docs/adr/0002-tauri-v2-shell.md docs/adr/0003-swift-core-sidecar.md
./shared/ci/rust-purity.sh
bash linux/ci/gates.sh --self-check
```

Refs #47 · #240 · #407.
