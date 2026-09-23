> **Status:** current — tracked risk register entry; recommendations await the owner's approval (charter §3).
الفحص الحقيقي (`cargo audit`) اشتغل النهارده: ٤ ثغرات أمنية و٦ تحذيرات (٤ مكتبات صيانتها وقفت، ١ فيها خلل أمان، ١ نسخة اتسحبت من المتجر).
التسع تحذيرات اللي Codex لقاها بالمراجعة اليدوية طلعت كلها صح، لكن فاته تحذير عاشر: نسخة مسحوبة من مكتبة اسمها chacha20.
ثغرتين منهم (quick-xml) في أدوات تجهيز نسخة لينكس بس، ومش داخلين خالص في نسخة ويندوز.
فيه ٧ تحديثات صغيرة متوافقة بتصلّح ٨ من العشرة؛ جرّبناها على نسخة منفصلة والفحص طلع صفر ثغرات، بس لسه محتاجة اختبارات وموافقتك.
مكتبات الخطوط (ttf-parser و rustybuzz) محتاجة شغل منفصل؛ نقترح قبول تحذيرات صيانتها مؤقتًا لحد مراجعة البدائل.
ما اتغيّرش أي كود أو مكتبة في المشروع؛ ده تقرير وخطة مستنية موافقتك.

# Cargo dependency security triage — 2026-09-23

## Evidence status and scope

**This report is now backed by a real `cargo audit` run** (cargo-audit 0.22.2, advisory database commit `1e640cd5`, 2026-09-23). The first draft was written by Codex by manual inspection because its sandbox had no network; a second session installed the tool, ran it, reproduced every dependency path with `cargo tree`, and corrected this file in place. It is still not a security clearance: no exploit reproduction or full interprocedural reachability analysis was performed.

**Real `cargo audit` summary (from `varos/`, exit 1):**

```text
    Scanning Cargo.lock for vulnerabilities (498 crate dependencies)
error: 4 vulnerabilities found!
warning: 6 allowed warnings found
```

The 6 warnings are 4 × unmaintained, 1 × unsound, 1 × yanked. The JSON report agrees (`vulnerabilities.count = 4`; warnings `unmaintained: 4`, `unsound: 1`, `yanked: 1`).

| Item | Evidence |
|---|---|
| Repository baseline | Draft written at `61786a0`; verified at `3877dd4` (docs-only commit in between; `Cargo.lock` unchanged). |
| Workspace examined | `varos/Cargo.toml`; four first-party crates and the patched `varos/vendor/egui_tiles`. `varos-spike` is outside this workspace and was not audited. |
| Toolchain | `cargo 1.98.1 (797e8a9bc 2026-08-05)`; `cargo-audit-audit 0.22.2` (installed with `cargo install cargo-audit --locked`). |
| Lockfile SHA-256 | `f6def878b7f71615c344263d4d21e52544100f1739c851bd4cf230fe10a875c6` — identical before and after every command in this report. |
| Advisory DB | 1267 advisories, commit `1e640cd56d7604993e3a9ec392060666e3b95ccc`, last updated `2026-09-23T16:07:49+02:00`. |
| Audit config | No `varos/.cargo/audit.toml`, no `~/.cargo/audit.toml`; JSON `settings.ignore = []`, `informational_warnings = ["unmaintained","unsound","notice"]`. Nothing is suppressed. |
| Historical count | `docs/foundation/STATUS.md:61` (was `:39` at `61786a0`): `3 vulns + 3 unmaintained (untriaged)`. Superseded by today's run: 4 vulns + 4 unmaintained + 1 unsound + 1 yanked. |
| Writes | This report only. No `Cargo.toml`, `Cargo.lock` or source change. Candidate bumps were tested only with `cargo update --dry-run` and on a throw-away copy of `varos/` outside the repository. |

## Advisory register (verified)

Verdict column: **Confirmed** = Codex's row matches the real tool (crate, version, type, severity, fix); **Corrected** = row kept but a detail fixed; **Added** = the real tool reports it and Codex missed it. No row was **Removed** — Codex invented nothing.

"Unspecified" means the advisory database has no CVSS score; it does not mean low severity. None of these packages is used directly by Varos code (first-party search below).

| Verdict | RUSTSEC ID | Locked crate | Type / severity (from `cargo audit`) | Plain-language meaning | Fixed version / removal route |
|---|---|---|---|---|---|
| Confirmed | [RUSTSEC-2026-0204](https://rustsec.org/advisories/RUSTSEC-2026-0204) | `crossbeam-epoch 0.9.18` (`varos/Cargo.lock:733`) | Vulnerability; unspecified | Formatting certain invalid/null pointers can dereference them (undefined behavior). | `>=0.9.20`. Dry-run resolves to **0.9.21** within existing ranges. |
| Confirmed | [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194) | `quick-xml 0.39.4` (`varos/Cargo.lock:2905`) | Vulnerability; **High 7.5** (`AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H`) | A tag with many attributes can burn excessive CPU. | `>=0.41.0` via `wayland-scanner 0.31.10 → 0.31.11` (dry-run confirms quick-xml → 0.41.0). |
| Confirmed | [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195) | `quick-xml 0.39.4` (`varos/Cargo.lock:2905`) | Vulnerability; **High 7.5** (same vector) | Namespace parsing can allocate excessive memory. | Same scanner bump. |
| Confirmed | [RUSTSEC-2026-0257](https://rustsec.org/advisories/RUSTSEC-2026-0257) | `webbrowser 1.2.1` (`varos/Cargo.lock:4267`) | Vulnerability; unspecified | Under Linux/BSD `BROWSER` settings, a crafted non-HTTP(S) URL can inject browser arguments. | `>=1.2.2`. Dry-run resolves to **1.2.4** (Codex said 1.2.2 "or later 1.2.x" — consistent). |
| Confirmed | [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370) | `proc-macro-error 1.0.4` (`varos/Cargo.lock:2832`) | Warning: unmaintained | Compiler-macro helper no longer maintained. | No patched version. `flo_curves 0.8.0 → 0.8.1` (→ `ouroboros 0.18.5`) removes it — dry-run confirmed. |
| Confirmed | [RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173) | `proc-macro-error2 2.0.1` (`varos/Cargo.lock:2866`) | Warning: unmaintained | Replacement fork also unmaintained. | No patched version. `defmt 1.1.0 → 1.1.1` (+ `defmt-macros 1.1.1`) removes it — dry-run confirmed. |
| Confirmed | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192) | `ttf-parser 0.25.1` (`varos/Cargo.lock:3785`) | Warning: unmaintained | Font parser will not get further fixes. | No patched version. Needs parent migrations (resvg, lopdf, Wayland decorations) — gated work. |
| Confirmed | [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206) | `rustybuzz 0.20.1` (`varos/Cargo.lock:3189`) | Warning: unmaintained | Older text-shaping engine no longer maintained. | No patched version. Needs `resvg 0.45 → 0.48` migration — gated work. |
| Confirmed | [RUSTSEC-2026-0221](https://rustsec.org/advisories/RUSTSEC-2026-0221) | `event-listener 5.4.1` (`varos/Cargo.lock:1099`) | Warning: **unsound**, no CVSS | Stack listeners can move a non-thread-safe tag across threads (data race). | `>=5.4.2` (unaffected `<5.1.0`). Dry-run resolves to 5.4.2. |
| **Added** (Codex missed) | — (not a RUSTSEC advisory) | `chacha20 0.10.1` (`varos/Cargo.lock:548`) | Warning: **yanked** | The publisher withdrew this release from crates.io (0.10.0 and 0.10.1 both yanked; 0.10.2 is not). Yanked ≠ known vulnerability, but it signals a release the author does not want used. | `chacha20 0.10.1 → 0.10.2` — dry-run confirmed, one package, no other change. |

Why Codex missed `chacha20`: yanked status is not in the advisory database; it comes from the live crates.io index, which Codex could not reach. (The first real run in this session also missed it because another process held the Cargo package-cache lock — `warning: couldn't update crates.io index` — so the audit was re-run until the index check succeeded; see Verification.)

The crossbeam advisory's title says `fmt::Pointer` while its body says `fmt::Display` — Codex's note on this is **confirmed** against the advisory text in the database.

## Parent chains and reachable behavior

All chains below are now **real `cargo tree --locked -i <crate>` output** (not lockfile reconstruction), run for three scopes: host (`aarch64-apple-darwin`), `--target all`, and `--target x86_64-pc-windows-msvc` (our real product target). Arrows run from our crate toward the affected dependency.

| Finding | Verified chain(s) into our crates | In Windows build? | Reachability assessment (verdict on Codex's reasoning) |
|---|---|---|---|
| `0204` crossbeam-epoch | `varos-app → varos-pdf → lopdf 0.43.0 → rayon 1.12.0 → rayon-core 1.13.0 → crossbeam-deque 0.8.6 → crossbeam-epoch 0.9.18` | **Yes** | **Confirmed.** Runtime dependency of PDF loading (`varos/crates/varos-app/src/main.rs:420` → `varos/crates/varos-pdf/src/lib.rs:37`, `:344` `lopdf::Document::load_mem`). The bug needs pointer *formatting*, not parsing; no trigger demonstrated. Patch anyway. |
| `0194`, `0195` quick-xml | Only parent is `wayland-scanner 0.31.10 (proc-macro)`, reached via `winit 0.30.13` (sctk-adwaita / smithay-client-toolkit / wayland-protocols*), `egui-winit 0.35.0 → smithay-clipboard 0.7.3`, and `rfd 0.15.4 → ashpd 0.11.1 → wayland-client` | **No** (Windows and macOS trees print nothing) | **Confirmed, and strengthened.** `cargo tree` proves it is absent from the Windows build; it is a build-time code generator for Linux/Wayland only. Codex's "winit / egui-winit → Wayland crates" summary is correct; the exact `egui-winit → smithay-clipboard 0.7.3 → smithay-client-toolkit 0.20.0` link is now spelled out. |
| `0257` webbrowser | `varos-app → egui-winit 0.35.0 → webbrowser 1.2.1` | **Yes (crate compiled), vulnerable code not** | **Confirmed, with precision added.** The crate *is* in the Windows build, but `webbrowser-1.2.1/src/lib.rs:42-65` selects `windows.rs` on Windows and `macos.rs` on macOS; the vulnerable `unix.rs` is only compiled for Linux/BSD. The only bridge is `varos/crates/varos-app/src/ui.rs:1346` (`handle_platform_output`); no first-party `open_url`/hyperlink producer. |
| `0370` proc-macro-error | `varos-core → flo_curves 0.8.0 → ouroboros 0.17.2 → ouroboros_macro 0.17.2 (proc-macro) → proc-macro-error 1.0.4`; core feeds `varos-app`, `varos-pdf`, `varos-render-wgpu` | Yes (build time only) | **Confirmed.** Compile-time macro helper; not a drawing-file exploit, but still a supply-chain maintenance risk. **Corrected citation:** `flo_curves = "0.8"` is at `varos/crates/varos-core/Cargo.toml:12`, not `:11`. |
| `0173` proc-macro-error2 | Lockfile only: `lopdf → jiff 0.2.31 → defmt 1.1.0 → defmt-macros 1.1.0 → proc-macro-error2`. **`cargo tree` prints nothing for all three scopes.** | **No — not compiled on any target** | **Confirmed, and now proven.** Codex said "optional, not demonstrated active"; the real feature-resolved tree shows it is never built with current features. It is flagged only because it sits in `Cargo.lock`. |
| `0192` ttf-parser | (1) `varos-app → resvg 0.45.1 → usvg 0.45.1 → fontdb 0.23.0 / rustybuzz 0.20.1 → ttf-parser`; (2) `varos-app → varos-pdf → lopdf 0.43.0 → ttf-parser`; (3) `winit / egui-winit → sctk-adwaita 0.10.1 → ab_glyph 0.2.32 → owned_ttf_parser 0.25.1 → ttf-parser` | Yes — paths (1) and (2); path (3) is Linux-only | **Confirmed.** Maintenance risk, no exploit alleged. SVG callers: `varos/crates/varos-app/src/cursors.rs:209,248,277`, `ui.rs:852,864`. egui/epaint is not a parent. |
| `0206` rustybuzz | `varos-app → resvg 0.45.1 → usvg 0.45.1 → rustybuzz 0.20.1` | Yes | **Confirmed.** Used via our own UI/cursor SVGs with default `usvg::Options`; arbitrary document text shaping not shown. |
| `0221` event-listener | `varos-app → rfd 0.15.4 → ashpd 0.11.1 → zbus 5.16.0 / async-fs / async-net → async-lock / async-process / async-broadcast / event-listener-strategy / async-channel / blocking → event-listener 5.4.1` | **No** (Windows and macOS trees print nothing) | **Confirmed.** `rfd-0.15.4/Cargo.toml:86` pulls `ashpd` only on Linux/BSD. Patch anyway. |
| yanked chacha20 | `varos-app → varos-pdf → lopdf 0.43.0 → rand 0.10.2 → chacha20 0.10.1` | **Yes** | **Added.** Runtime dependency of the PDF library. No advisory; recommend the one-package bump. |

First-party negative-use search, re-run 2026-09-23 (added `chacha|rand::`):

```sh
rg -n 'crossbeam|quick_xml|proc_macro_error|ttf_parser|rustybuzz|event_listener|webbrowser|hyperlink|open_url|handle_platform_output|chacha|rand::' varos/crates varos/vendor/egui_tiles/src --glob '*.rs'
```

Sole match: `varos/crates/varos-app/src/ui.rs:1346` (platform-output bridge). All other first-party line citations in this report were spot-checked against HEAD and match. Negative searches do not prove whole-program unreachability.

## Minimal proposed fix plan

Nothing here was applied to the repository. All changes await Ahmed's approval.

### A. One bounded patch-resolution commit candidate — now pre-checked

| Proposed change | Verified by | Advisory effect |
|---|---|---|
| `crossbeam-epoch 0.9.18 → 0.9.21` | dry-run | removes `0204` |
| `event-listener 5.4.1 → 5.4.2` | dry-run | removes `0221` |
| `webbrowser 1.2.1 → 1.2.4` | dry-run | removes `0257` |
| `wayland-scanner 0.31.10 → 0.31.11` (pulls `quick-xml 0.39.4 → 0.41.0`) | dry-run | removes `0194`, `0195` |
| `flo_curves 0.8.0 → 0.8.1` (`ouroboros 0.17.2 → 0.18.5`) | dry-run | removes `0370` |
| `defmt 1.1.0 → 1.1.1` + `defmt-macros 1.1.1` | dry-run | removes `0173` |
| **new:** `chacha20 0.10.1 → 0.10.2` | dry-run | removes yanked warning |

Exact dry-run command (does not write `Cargo.lock`; hash re-checked unchanged):

```sh
cargo update --dry-run -p crossbeam-epoch -p event-listener -p webbrowser -p wayland-scanner -p flo_curves -p defmt
cargo update --dry-run -p chacha20
```

Side effects the dry-run shows (review in the lockfile diff): removes `core-foundation 0.10.1`, `itertools 0.11.0`, `proc-macro-error(-attr) 1.0.4`, `proc-macro-error-attr2 2.0.0`, `proc-macro-error2 2.0.1`, `syn 1.0.109`; adds `proc-macro2-diagnostics 0.10.1`, `yansi 1.0.1`. No `Cargo.toml` change needed — current ranges already admit all seven.

**Candidate audit result (throw-away copy of `varos/` outside the repo, all seven bumps applied, `cargo audit` exit 0):**

```text
    Scanning Cargo.lock for vulnerabilities (493 crate dependencies)
warning: 2 allowed warnings found
```

Remaining: only `RUSTSEC-2026-0192` (ttf-parser) and `RUSTSEC-2026-0206` (rustybuzz). This proves the *audit* effect only. **Nothing was compiled or tested with these versions.** `flo_curves 0.8.1` is the one direct dependency among them and touches Boolean geometry — it may deserve its own commit if tests show any change.

Approval exit criteria (unchanged from the draft): inspect the exact lockfile diff; run `cargo audit` + JSON again; `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`; Ahmed hand-tests Windows startup, drawing/Boolean operations, Open/Save, dialogs, text, and cursor artwork.

### B. Proposed temporary acceptance, with explicit expiry

Prefer fixing over ignoring anything patchable. After section A, only the two font maintenance warnings remain. Proposal only, for a future `varos/.cargo/audit.toml` after owner approval (no ignore file exists today). Owner: Ahmed; review by **2026-10-23**, earlier on external testing, SVG/font import, a new related advisory, or a parent dependency change. cargo-audit does not enforce comment expiry.

```toml
[advisories]
informational_warnings = ["unmaintained", "unsound", "notice"]
ignore = [
    # Proposed, NOT approved. Owner: Ahmed. Review by 2026-10-23.
    # Maintenance warning, no exploit alleged. Parents: resvg/usvg, lopdf,
    # and (Linux only) Wayland decoration fonts. See this risk register.
    "RUSTSEC-2026-0192", # ttf-parser
    # Proposed, NOT approved. Owner: Ahmed. Review by 2026-10-23.
    # UI/cursor SVG path only; replacement needs a gated resvg migration.
    "RUSTSEC-2026-0206", # rustybuzz
]

[output]
deny = ["unmaintained", "unsound", "notice"]
```

Do not ignore `0370`/`0173` automatically — their removal bumps exist and were dry-run verified. Never "fix" proc-macro-error by switching to proc-macro-error2: both are flagged. Note `deny` above does not include `yanked`; keep the chacha20 bump rather than relying on config.

### C. Gated migrations and upstream waiting

For pre-1.0 crates, `0.45 → 0.48` is a compatibility-breaking release. These were not re-verified beyond what `cargo tree` and the dry-run show; they remain Codex's analysis, consistent with the real trees.

| Top-level dependency / locked version | Diagnosis and approved-work boundary |
|---|---|
| `wgpu 29.0.4` | No advisory requires moving. Any major migration **needs its own gated piece**. |
| `egui`, `egui-wgpu`, `egui-winit 0.35.0` | webbrowser fixed within range (1.2.4). Any `0.35 → 0.36+` **needs its own gated piece**. |
| `winit 0.30.13` | Scanner fix available within range. Any windowing migration **needs its own gated piece**. |
| `rfd 0.15.4` | event-listener and scanner fixes need no direct bump. `0.16+` **needs its own gated piece**. |
| `resvg 0.45.1` | Moving to 0.48 (harfrust/skrifa) is the route off rustybuzz and one ttf-parser path — **separate gated piece** with cursor/icon output comparisons. |
| `lopdf 0.43.0` | crossbeam and chacha20 are patchable independently. `0.43 → 0.45` (skrifa instead of ttf-parser) **needs its own gated piece** with `.vrs` round-trip checks. |
| `image 0.25.10`, `windows 0.62.2`, vendored `egui_tiles 0.16.0` | No finding requires a change. Any bump **needs its own gated piece** (`docs/VENDOR_PATCHES.md` for the fork). |

**Upstream-wait item:** the Linux-only `sctk-adwaita → ab_glyph 0.2.32 → owned_ttf_parser 0.25.1 → ttf-parser` path. Even after resvg and lopdf migrations, the all-target audit keeps `0192` until that chain changes. Recheck monthly and before releases.

## Standing local gate

`.github/workflows/ci.yml:28` runs `cargo install cargo-audit --locked && cargo audit`, and `:29` (`continue-on-error: true`) makes it non-blocking; this report does not change CI. The owner should run the local gate before accepting any dependency commit, before a release, and at least monthly.

1. cargo-audit 0.22.2 is now installed at `~/.cargo/bin/cargo-audit`; record `cargo audit --version` each time, do not reinstall implicitly.
2. From `varos/`, capture `cargo audit` and `cargo audit --json` with separate exit codes, tool version, advisory DB commit/date, and `Cargo.lock` hash. Audit all targets.
3. **Check the output for `warning: couldn't update crates.io index`.** If present, the yanked-crate check was skipped (this happened once today because of a concurrent Cargo lock) — re-run; that result is **BLOCKED**, never PASS. Same for a missing tool, network failure, stale DB, malformed JSON, or an unexplained new finding.
4. For every new finding run `cargo tree --locked -i <crate>` with `--target all` and `--target x86_64-pc-windows-msvc`.
5. The audit gate passes only after fixes are verified and remaining exceptions are explicitly approved. Code/test gates and Ahmed's real-window test stay separate.

## Verification

- **Date:** 2026-09-23 (≈21:40–21:57 EEST).
- **Tool:** `cargo-audit-audit 0.22.2` (`cargo install cargo-audit --locked`, `Installed package cargo-audit v0.22.2`, build 15m 38s), cargo 1.98.1.
- **Advisory DB:** 1267 advisories, commit `1e640cd56d7604993e3a9ec392060666e3b95ccc` (2026-09-23T16:07:49+02:00).
- **Lockfile:** 498 crate dependencies, SHA-256 `f6def878…a875c6`, unchanged throughout.
- **Commands (from `varos/`):**
  ```sh
  cargo install cargo-audit --locked
  cargo audit --version
  cargo audit                              # exit 1
  cargo audit --json > /tmp/varos-audit.json   # exit 1
  cargo tree --locked -i <crate>                                   # host
  cargo tree --locked --target all -i <crate>
  cargo tree --locked --target x86_64-pc-windows-msvc -i <crate>
  # for crate in: crossbeam-epoch quick-xml proc-macro-error proc-macro-error2
  #               ttf-parser rustybuzz event-listener webbrowser chacha20
  cargo update --dry-run -p crossbeam-epoch -p event-listener -p webbrowser -p wayland-scanner -p flo_curves -p defmt
  cargo update --dry-run -p chacha20
  ```
  Candidate-bump audit ran on an `rsync` copy of `varos/` in a temporary directory, never in the repository.
- **Raw summary lines (authoritative run):**
  ```text
  error: 4 vulnerabilities found!
  warning: 6 allowed warnings found
  ```
- **First run note:** the first `cargo audit` in this session printed `warning: couldn't update crates.io index: unable to acquire filesystem lock` (other Cargo jobs were running) and reported `5 allowed warnings` — the yanked check was skipped. The re-run with the index reachable is the authoritative one above.
- **Evidence files (local, not committed):** `/tmp/varos-audit.json` (SHA-256 `9fef8fa8…fe7d1d`), `/tmp/varos-audit.txt`, `/tmp/varos-audit-trees.txt`, `/tmp/varos-audit-candidate-bumps.txt`.
- **Scorecard vs. Codex draft:** 9 of 9 advisory rows confirmed (IDs, crates, versions, types, severities, fix versions all correct); 0 invented; 1 finding missed (yanked `chacha20 0.10.1`); 2 small corrections (`flo_curves` citation `:11 → :12`; STATUS.md line moved `:39 → :61` by a later commit). Codex's reachability conclusions all held; the `proc-macro-error2` "not active" and quick-xml/event-listener "not on Windows" conclusions are now proven by `cargo tree`, not just inferred.

## Appendix A — full `cargo audit` text output (authoritative run)

```text
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1267 security advisories (from /Users/gomaa/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (498 crate dependencies)
Crate:     crossbeam-epoch
Version:   0.9.18
Title:     Invalid pointer dereference in `fmt::Pointer` impl for `Atomic` and `Shared` when the underlying pointer is invalid
Date:      2026-07-06
ID:        RUSTSEC-2026-0204
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0204
Solution:  Upgrade to >=0.9.20

Crate:     quick-xml
Version:   0.39.4
Title:     Unbounded namespace-declaration allocation in `NsReader` enables memory-exhaustion denial of service
Date:      2026-06-29
ID:        RUSTSEC-2026-0195
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0195
Severity:  7.5 (high)
Solution:  Upgrade to >=0.41.0

Crate:     quick-xml
Version:   0.39.4
Title:     Quadratic run time when checking a start tag for duplicate attribute names
Date:      2026-06-29
ID:        RUSTSEC-2026-0194
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0194
Severity:  7.5 (high)
Solution:  Upgrade to >=0.41.0

Crate:     webbrowser
Version:   1.2.1
Title:     Unix `BROWSER` handling allows browser argument injection
Date:      2026-07-29
ID:        RUSTSEC-2026-0257
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0257
Solution:  Upgrade to >=1.2.2

Crate:     proc-macro-error
Version:   1.0.4
Warning:   unmaintained
Title:     proc-macro-error is unmaintained
Date:      2024-09-01
ID:        RUSTSEC-2024-0370
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0370

Crate:     proc-macro-error2
Version:   2.0.1
Warning:   unmaintained
Title:     proc-macro-error2 is unmaintained
Date:      2026-06-07
ID:        RUSTSEC-2026-0173
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0173

Crate:     rustybuzz
Version:   0.20.1
Warning:   unmaintained
Title:     `rustybuzz` is unmaintained
Date:      2026-07-11
ID:        RUSTSEC-2026-0206
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0206

Crate:     ttf-parser
Version:   0.25.1
Warning:   unmaintained
Title:     `ttf-parser` is unmaintained
Date:      2026-06-28
ID:        RUSTSEC-2026-0192
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0192

Crate:     event-listener
Version:   5.4.1
Warning:   unsound
Title:     `event-listener` allows `!Send` tags to cross thread boundaries via `StackSlot`
Date:      2026-07-13
ID:        RUSTSEC-2026-0221
URL:       https://rustsec.org/advisories/RUSTSEC-2026-0221

Crate:     chacha20
Version:   0.10.1
Warning:   yanked

error: 4 vulnerabilities found!
warning: 6 allowed warnings found
```

## Appendix B — history of the Codex draft

Codex's original draft (written at `61786a0`) could not reach crates.io: `cargo install cargo-audit --locked` failed with `Couldn't resolve host name (Could not resolve host: index.crates.io)`, every `cargo audit` call failed with `error: no such command: audit`, and every `cargo tree --offline` failed on missing cached packages (`failed to download ab_glyph v0.2.32`). It therefore built the register from RustSec pages, `Cargo.lock` edges, and cached registry sources. That failure transcript is superseded by the real outputs above and is not repeated here (the draft was never committed). Codex also correctly excluded three non-matching advisories by version (`lopdf 0.43.0` ≥ fixed `0.42.0` for RUSTSEC-2026-0187; `memmap2 0.9.11` at fixed version for RUSTSEC-2026-0186; `i_tree 0.19.0` ≥ fixed `0.10.0` for RUSTSEC-2025-0165) — the real audit reports none of them, consistent with that.

## Approval record

**Owner decision: pending.** No exception or upgrade is approved by this report. Next step for Ahmed: approve (or not) the seven-bump patch commit in section A; separately decide the temporary font-warning acceptance in B; the font/UI migrations in C each stay their own gated piece.
