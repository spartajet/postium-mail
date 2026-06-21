# Postium Mail E2E Stabilization Design

Date: 2026-06-21
Status: Approved for planning

## Goal

Stabilize the existing Tauri E2E framework so it can be used as a reliable Linux CI gate and a repeatable local regression suite.

This work upgrades the current WebdriverIO + tauri-driver setup from a smoke harness into a stable test platform with isolated data, deterministic fixtures, stable selectors, independent specs, and useful failure artifacts.

## Scope

In scope:

- Run E2E tests against an isolated per-run app data directory.
- Enable a dedicated E2E runtime mode with `POSTIUM_E2E=1`.
- Seed deterministic local data at app startup in E2E mode.
- Seed at least 2 accounts and at least 48 emails.
- Add only the `data-testid` anchors needed by E2E tests.
- Rewrite page objects to use stable selectors and explicit waits.
- Rewrite current E2E specs so tests are independent and do not silently skip required actions.
- Add one account-switching test and one email-list scrolling test.
- Make Linux E2E the required CI target for this phase.
- Capture screenshots, logs, reports, and failed run data directories.
- Document local E2E setup and troubleshooting.

Out of scope:

- Real IMAP, SMTP, OAuth, or remote mail-provider tests.
- Full Rust command/service test expansion.
- Full Vitest store/component test expansion.
- Windows E2E stabilization as a required CI gate.
- Large business-flow expansion beyond stabilizing the current suite plus account switching and scrolling.
- Removing `data-testid` attributes from production DOM.

## Design Principles

Default E2E tests should use the real internal application chain and avoid external services.

The following should be real:

- Tauri app startup
- Svelte UI
- Tauri commands
- Rust services
- SQLite database
- Frontend stores and routing

The following should be avoided in default E2E:

- Real IMAP servers
- Real SMTP sending
- Real OAuth flows
- Real system keyring dependencies
- Live network mail-provider behavior

The test data source is deterministic local seed data, not mocked frontend API responses. This preserves E2E value while removing unstable external dependencies.

## Runtime Architecture

The E2E run uses a dedicated data path controlled by environment variables.

```text
WDIO onPrepare
  -> create .e2e-data/run-<timestamp>-<pid>
  -> set POSTIUM_E2E=1
  -> set POSTIUM_DATA_DIR=<absolute run dir>
  -> build Tauri debug app

tauri-driver
  -> start the app binary

Tauri app run()
  -> detect POSTIUM_E2E=1
  -> require POSTIUM_DATA_DIR
  -> init_database(POSTIUM_DATA_DIR)
  -> run migrations
  -> seed_e2e_data(db)
  -> start normal app services and window

WDIO specs
  -> wait for app readiness
  -> interact through stable data-testid selectors
  -> assert deterministic seed state
```

## Data Directory Resolution

Rust startup should stop hard-coding `~/.postium` directly inside `run()`.

Introduce a small resolver:

```rust
fn resolve_data_dir() -> PathBuf
```

Behavior:

- Normal mode: use the current `~/.postium` path.
- E2E mode: if `POSTIUM_E2E=1`, use `POSTIUM_DATA_DIR`.
- E2E mode without `POSTIUM_DATA_DIR`: fail startup with a clear message.

This keeps production behavior unchanged while making E2E data isolation explicit.

## E2E Seed Data

Seed data is inserted by Rust at app startup only when `POSTIUM_E2E=1`.

Recommended module location:

```text
src-tauri/src/infrastructure/testing/e2e_seed.rs
```

The seed module should be idempotent:

- Check for the primary test account.
- If it exists, assume seed has already run and skip insertion.
- If it does not exist, insert all E2E accounts and emails.

The seed should use existing repository/entity patterns where practical. Raw SQL should be avoided unless needed for schema-specific behavior.

### Accounts

Insert 2 fixed accounts:

Primary account:

- email: `primary.e2e@postium.test`
- name: `Primary E2E`
- provider: `custom`
- auth_type: `Password`
- sync_enabled: `false`
- color: `#2563eb`

Secondary account:

- email: `secondary.e2e@postium.test`
- name: `Secondary E2E`
- provider: `custom`
- auth_type: `Password`
- sync_enabled: `false`
- color: `#16a34a`

### Emails

Insert at least 48 fixed emails.

Primary account: 36 emails

- Inbox: 24
- Sent: 5
- Starred: 4
- Drafts: 2
- Trash: 1

Secondary account: 12 emails

- Inbox: 8
- Sent: 2
- Starred: 1
- Drafts: 1

The seed must use fixed subjects, senders, bodies, folder values, read states, star states, and timestamps. Timestamps should be deterministic and sorted so list ordering is predictable.

Required subject examples:

- `Primary Inbox Message 01`
- `Primary Inbox Message 24`
- `Secondary Inbox Message 01`
- `Quarterly Planning Alpha`
- `Quarterly Planning Beta`
- `Quarterly Planning Archive`
- `Starred Reference Message`
- `Sent Confirmation Message`
- `Draft Proposal Outline`
- `Trash Cleanup Notice`

The first scrolling test should use `Primary Inbox Message 24` as the bottom-list target.

## Selector Strategy

E2E selectors should not depend on Tailwind classes, DOM position, or translated UI text.

Priority:

1. `data-testid`
2. stable `aria-label`
3. role/name
4. visible text for seed-content assertions only

Required anchors for this phase:

Sidebar:

- `sidebar`
- `compose-button`
- `folder-inbox`
- `folder-sent`
- `folder-starred`
- `settings-nav`
- `account-switcher`
- `account-option-primary`
- `account-option-secondary`
- `active-account-label`

Email list:

- `email-list`
- `email-search-input`
- `email-item`
- `email-empty-state`
- `email-refresh-button`

Email detail:

- `email-detail`
- `email-detail-empty`
- `email-subject`
- `email-sender`
- `email-body`
- `email-star-button`
- `email-delete-button`

Compose modal:

- `compose-modal`
- `compose-to-input`
- `compose-cc-input`
- `compose-subject-input`
- `compose-body-editor`
- `compose-send-button`
- `compose-close-button`

Settings:

- `settings-page`
- `theme-light`
- `theme-dark`
- `theme-system`

## Page Object Design

Add a shared selector helper:

```text
e2e/helpers/selectors.js
```

Example:

```js
export const byTestId = (id) => $(`[data-testid="${id}"]`);
export const allByTestId = (id) => $$(`[data-testid="${id}"]`);
```

Page objects should:

- Use `data-testid` as the main selector mechanism.
- Expose `waitForReady()` methods for major UI regions.
- Use `waitForDisplayed` and `browser.waitUntil`.
- Fail when required elements are missing.
- Avoid `if (element) { ... }` around required UI.
- Avoid `browser.pause()` as the primary synchronization mechanism.
- Avoid selecting inputs by position.

## Spec Design

Rewrite current specs around deterministic state and independent test setup.

Recommended specs:

```text
e2e/test/specs/smoke.e2e.js
e2e/test/specs/navigation.e2e.js
e2e/test/specs/account-switching.e2e.js
e2e/test/specs/email-list.e2e.js
e2e/test/specs/compose.e2e.js
e2e/test/specs/theme.e2e.js
```

Smoke:

- App launches.
- Sidebar appears.
- Email list appears.
- Primary seed account is active.
- Primary inbox seed mail is visible.

Navigation:

- Inbox shows primary inbox messages.
- Sent shows primary sent messages.
- Starred shows starred messages.
- Required folder buttons are visible and clickable.

Account switching:

- Default active account is primary.
- Open account switcher.
- Switch to secondary.
- Verify `Secondary Inbox Message 01` appears.
- Verify primary-only inbox message is not displayed.
- Switch back to primary.
- Verify primary inbox messages appear.

Email list:

- Search for `Quarterly Planning`.
- Verify deterministic matching subjects appear.
- Clear search and verify normal inbox returns.
- Scroll the email list to `Primary Inbox Message 24`.
- Click it and verify the detail pane shows the same subject.

Compose:

- Open compose modal.
- Fill recipient, subject, and body.
- Verify values are entered.
- Close modal.
- Reopen modal in a fresh test and verify it starts from clean state.

Theme:

- Navigate to settings.
- Switch to dark and verify `html.dark`.
- Switch to light and verify `html.dark` is removed.
- Click system theme and verify the control is usable.

## WDIO Lifecycle

`e2e/wdio.conf.js` should manage the test runtime explicitly.

Responsibilities:

- Create `.e2e-data/run-<timestamp>-<pid>`.
- Pass `POSTIUM_E2E=1` and `POSTIUM_DATA_DIR` to the Tauri build/app process.
- Use `WDIO_PORT` with default `4444`.
- Build the debug app before tests.
- Check that the app binary exists after build.
- Start `tauri-driver`.
- Wait for `127.0.0.1:<port>` to accept connections.
- Kill `tauri-driver` on completion or interruption.
- Delete the run data directory on success.
- Preserve the run data directory on failure.

## Failure Artifacts

Create:

```text
e2e/artifacts/
├── screenshots/
├── logs/
└── reports/
```

Capture:

- Screenshot for each failed test.
- WDIO report output.
- Driver and app logs where practical.
- Failed `.e2e-data/run-*` directory.

Successful runs may clean the run data directory. Failed runs should keep it for local investigation and CI upload.

## CI Design

Linux E2E is the required target for this phase.

Recommended CI structure:

- Rust tests
- Frontend tests
- Linux E2E tests

Windows E2E should not be a required gate in this phase. It can be removed from the required matrix or moved to a separate non-blocking job. The recommended first step is to remove Windows from the required E2E matrix and restore it in a later stabilization phase.

Linux E2E should:

- Install Tauri Linux dependencies.
- Install `webkitgtk-webdriver` and `xvfb`.
- Install `tauri-driver`.
- Run E2E under `xvfb-run`.
- Upload `e2e/artifacts/**` always.
- Upload `.e2e-data/**` on failure.

## Documentation

Add local E2E documentation, either in `README.md` or `docs/testing/e2e.md`.

Document:

- Required local dependencies.
- `cargo install tauri-driver --locked`.
- `bun install` in root and `e2e`.
- `bun run test:e2e`.
- E2E environment variables.
- Artifact locations.
- How to inspect a failed `.e2e-data` directory.
- Linux CI is the only required E2E gate for this phase.

## Risks

Seed/schema mismatch:

- Mitigation: use repository/entity paths where practical and keep seed fields explicit.

Folder semantics mismatch:

- Mitigation: assert only currently supported folder/category behavior in this phase.

`data-testid` in production DOM:

- Mitigation: accept this for stability in phase one; optional stripping can be considered later.

Scrolling flakiness:

- Mitigation: test only that the bottom target becomes visible and clickable, not exact pixel positions.

Linux display environment differences:

- Mitigation: continue using `xvfb-run`; defer Windows stabilization.

## Acceptance Criteria

- Local E2E does not read or write `~/.postium`.
- Local E2E uses a run-specific `.e2e-data/run-*` directory.
- E2E mode fails clearly if `POSTIUM_E2E=1` and `POSTIUM_DATA_DIR` is missing.
- Seed creates at least 2 accounts.
- Seed creates at least 48 emails.
- Primary account has at least 24 inbox emails.
- Existing E2E specs use `data-testid` as primary selectors.
- Tests do not silently skip required actions with `if (element)`.
- Tests do not depend on state left by earlier `it` blocks.
- There is an account-switching E2E test.
- There is an email-list scrolling E2E test.
- Failed E2E tests save screenshots.
- CI uploads E2E artifacts.
- CI uploads failed run data directories.
- Linux E2E passes as a required gate.
- Windows E2E is not required for this phase.

## Implementation Order

1. Add Rust data directory resolution for E2E mode.
2. Add Rust E2E seed module with 2 accounts and at least 48 emails.
3. Add required `data-testid` anchors.
4. Upgrade WDIO lifecycle and artifact handling.
5. Rewrite page objects around `data-testid` and explicit waits.
6. Rewrite specs for smoke, navigation, account switching, email list scrolling, compose, and theme.
7. Simplify CI to Linux-required E2E and artifact upload.
8. Add local E2E documentation.

