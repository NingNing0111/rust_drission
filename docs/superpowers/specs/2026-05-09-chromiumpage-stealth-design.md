# 2026-05-09 ChromiumPage stealth integration design

## Goal
Integrate stealth injection into `ChromiumPage` so that newly constructed pages inject the anti-detection script by default, while preserving a fully compatible opt-out path for callers that do not want stealth injection.

## Requirements
- Keep `ChromiumPage::new(config)` source-compatible with all existing call sites.
- Make stealth injection the default behavior for `ChromiumPage::new(config)`.
- Provide an explicit public constructor that skips stealth injection.
- Ensure the stealth toggle changes only injection behavior and nothing else about browser connection, tab selection, or page setup.
- Avoid changing `BrowserConfig` so browser launch settings remain separate from page initialization policy.

## API design
### Public API
- Keep `ChromiumPage::new(config: BrowserConfig) -> Result<Self, CdpError>` unchanged.
- Add `ChromiumPage::new_without_stealth(config: BrowserConfig) -> Result<Self, CdpError>`.

### Internal API
- Introduce a private helper in `src/chromium_page.rs` that centralizes the shared construction flow, e.g. `fn build(config: BrowserConfig, inject_stealth: bool) -> Result<Self, CdpError>`.
- Route both public constructors through the helper:
  - `new(config)` passes `true`
  - `new_without_stealth(config)` passes `false`

This keeps the public API explicit while guaranteeing that both construction paths remain behaviorally identical apart from stealth injection.

## Behavioral boundaries
- `ChromiumPage::new(config)` will continue to connect or launch the browser, select the first existing tab or create a new tab, and then inject stealth before returning.
- `ChromiumPage::new_without_stealth(config)` will perform the exact same browser and tab initialization steps but skip the stealth injection call.
- `ChromiumPage::connect(endpoint)` remains unchanged in this change set. Its current behavior is preserved to minimize compatibility and semantic risk for callers that attach to an existing browser session.
- No changes are made to `BrowserConfig` fields or builder methods.

## Error handling
- The default constructor continues to surface stealth injection failure as a constructor error, because the page would otherwise be returned in a partially configured state relative to the default contract.
- The opt-out constructor cannot fail due to stealth injection because it never calls the injector.
- Shared browser connection and tab creation errors continue to behave exactly as they do today.

## Testing and verification
- Confirm existing `ChromiumPage::new(config)` call sites compile without modification.
- Add focused tests around the shared construction split where practical.
- At minimum, verify that both public constructors exist and compile against the existing type signatures.
- If testability is limited by real browser setup, prefer verifying API compatibility and shared control flow over introducing broader refactors solely for test instrumentation.

## Scope exclusions
- No new stealth-related flags in `BrowserConfig`.
- No changes to `connect(endpoint)` behavior.
- No expansion of stealth injection to newly opened tabs beyond current explicit behavior.
- No broader refactor of page construction unrelated to this toggle.
