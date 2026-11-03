# Shopping Extension Build-Out — Price Tracking and Deals in One Tool

- [ ] Use this file as the review checklist for merging the current Prices tool and Shopping tool into a single built-in Shopping extension.
- [ ] End state: one toolbar tool named **Shopping**, one popover, and one full page at `ce://shopping`. There is no separate Prices tool, popover, or page.
- [ ] Work in phase order unless a phase is explicitly reprioritized; Phase 1 (combined shell and data) comes before the page, alerts, and deal verification that build on it.
- [ ] Check off items only after the code, persistence migration, tests, and manual native-app check for that item are all done.
- [ ] Keep the combined popover working after each phase; do not leave a half-migrated settings, toolbar-pin, or `prices.json` format between changes.

## Audit Summary — Why the Two Tools Feel Broken

### Price tracking today

- [ ] Recognize the current limit: `frontend/src/features/BuiltInExtensions/price/Price.tsx` is only a 320px popover with Track and Check buttons, a flat list, and no remove, target, search, history, or per-item action.
- [ ] Recognize the data limit: `ProductPrice` in `src-tauri/src/tools.rs` stores only the current price and one previous price, so no trend, sparkline, first-tracked comparison, or drop-over-time calculation is possible.
- [ ] Recognize the extraction limit: `parse_price` only understands JSON-LD `Product` blocks and `product:price:amount` / `product:price:currency` meta tags, so stores using `@graph`, `AggregateOffer`, microdata, or DOM-only prices report "No price on this page."
- [ ] Recognize the checking limit: `check_prices` only re-reads a tracked item when its exact URL matches an already-open, loaded tab; it never fetches the tracked URL itself, so most checks silently skip the item.
- [ ] Recognize the alert limit: `dropped` and `stockChanged` are one-shot booleans overwritten on the next check, with no target price, no notification, no re-arm rule, and no record of when anything changed.

### Shopping today

- [ ] Keep what already works: `grounded_coupons` and `grounded_offers` in `src-tauri/src/shopping.rs` drop invented coupon codes, unknown URLs, and offers that are not cheaper than the page price; do not weaken these rules.
- [ ] Recognize the current limit: `frontend/src/features/BuiltInExtensions/shopping/Shopping.tsx` can display deals but cannot save an offer, track the product, mark a code as tried, or apply a code anywhere.
- [ ] Recognize the search limit: `find_deals` runs only two DuckDuckGo queries (`{store} coupon code`, `{name} price`) at 8 results each, all parsed from one brittle HTML results page.
- [ ] Recognize the AI dependency: `find_deals` returns an error when no AI endpoint or key is configured, even though the search results and verbatim codes could still be shown without any AI.
- [ ] Recognize the key-safety limit behind that dependency: the shared AI key is stored as plaintext `assistantKey` in `settings.json`; the password field in `frontend/src/features/Settings/ToolsSection.tsx` only masks it on screen, `preserve_local` in `src-tauri/src/sync_conflicts.rs` re-inserts it into synced settings, and profile export in `src-tauri/src/profile_archive.rs` keeps it.
- [ ] Recognize the verification limit: offer prices come from search-result snippets and an AI summary; no offer page is ever visited to confirm the price, stock, variant, or shipping before it is presented as a better deal.

### Why one extension, not two

- [ ] Recognize the overlap: both tools already operate on the same thing — a product on a store page — and already share `ProductPrice`, page-price reading, and the store host.
- [ ] Recognize the user cost of the split: tracking lives in one popover, coupons and better prices in another, and neither knows what the other found, so a tracked product shows no deals and a good offer cannot become a tracked item without starting over.
- [ ] Recognize the combined opportunity: one product model (tracked item + price history + latest deals + coupon feedback) powers the popover, the `ce://shopping` page, background checks, and alerts.

## Phase 0 — AI Providers and Encrypted API Keys (Shopping Summaries Only)

- [ ] Scope AI narrowly: it is used only to summarize and rank Shopping search results. Price tracking, price checking, alerts, coupon extraction, and offer display must all work with no provider configured.

### Provider model

- [ ] Add one active AI provider setting with `#[serde(default)]` in `BrowserSettings` (`src-tauri/src/settings.rs`), mirrored as camelCase in `types/browser.ts`, with unknown or retired values normalized to a safe default in `BrowserSettings::normalize`.
- [ ] Support four provider presets, each remembering its own endpoint, model, and key so switching providers never overwrites another provider's setup:
  - [ ] OpenAI: base URL `https://api.openai.com/v1`, `POST /chat/completions`, `Authorization: Bearer <key>`, model list from `GET /models`; the key field is labelled `OPENAI_API_KEY`.
  - [ ] Claude: native Anthropic API at `https://api.anthropic.com/v1`, `POST /messages` with `x-api-key: <key>` and `anthropic-version: 2023-06-01`, required `max_tokens`, `system` as a top-level field, and text joined from the response `content[]` blocks; the key field is labelled `ANTHROPIC_API_KEY` / `CLAUDE_API_KEY`.
  - [ ] Custom: any OpenAI-compatible base URL, model, and optional key, preserving today's endpoint behavior for OpenRouter, LM Studio, gateways, and self-hosted servers.
  - [ ] Local / Ollama: defaults to `http://127.0.0.1:11434/v1` with model `llama3.2` and no key required; keep the existing `localhost_endpoint` key exemption.
- [ ] Keep `shoppingModel` as an optional override of the provider's default model and `shoppingRegion` unchanged.
- [ ] Add an optional environment-variable hint in Settings: if `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` is set and no stored key exists, say so in the UI instead of silently ignoring it; never display the variable's value.
- [ ] Migrate existing installs on first load: a stored endpoint/model pair becomes the Custom (or Local, when it points at localhost) provider, and no existing working Shopping setup changes behavior.
- [ ] Translate one internal request shape (system text, user text, optional model override, JSON-only flag, timeout) per provider in the dispatch that replaces the single-provider body of `chat()` in `src-tauri/src/tools.rs`; `find_deals` is its only caller.
- [ ] For Shopping's JSON replies, use provider JSON modes where supported and otherwise a "reply with one JSON object only" instruction plus the existing first-`{` to last-`}` extraction in `parse_reply`; keep the grounded coupon/offer filtering after parsing, for every provider.
- [ ] Map HTTP failures to plain-language errors: 401/403 = key rejected, 429 = rate limited, 404 = unknown model or endpoint, connection refused on a localhost endpoint = local server not running.
- [ ] Keep the Shopping timeout (90s today) in the internal request shape, and log nothing that includes keys, search-result text sent to the provider, or full provider responses.

### Encrypted key storage (Windows DPAPI)

- [ ] Add a Rust-only secrets module (for example `src-tauri/src/secrets.rs`) wrapping `CryptProtectData` and `CryptUnprotectData` from the `windows` crate, which already enables `Win32_Security_Cryptography` in `src-tauri/Cargo.toml`; add no new crate for this.
- [ ] Store only base64 DPAPI ciphertext, keyed by provider id, in a machine-local file under the local app data directory resolved in `src-tauri/src/paths.rs` (beside the WebView2 profile), never in the profile data folder that a sync directory can relocate.
- [ ] Scope stored keys to the active browser profile so two profiles on one Windows account do not share keys.
- [ ] Decrypt keys only in Rust at request time and keep them out of command arguments, command return values, events, logs, and error strings; add a test that no request URL ever contains a key, extending the existing `chat_url` test style.
- [ ] Expose only key metadata to the frontend through new three-piece IPC (Rust command in `src-tauri/src/lib.rs`, camelCase type in `types/browser.ts`, wrapper in `frontend/src/lib/browser-client.ts`): set key, clear key, and get key status returning `hasKey` plus a masked hint of the last four characters.
- [ ] Add Test connection and List models commands that run entirely in Rust using the decrypted key, returning only success, a plain-language error, or model ids.
- [ ] On first load after this change, move any existing plaintext `assistantKey` into DPAPI storage, then rewrite `settings.json` without it.
- [ ] Remove the `assistantKey` re-insertion from `preserve_local` in `src-tauri/src/sync_conflicts.rs` so synced settings can never carry a key again.
- [ ] Strip keys from profile exports in `src-tauri/src/profile_archive.rs` the same way sync fields are stripped, and update its existing test that currently expects `assistantKey` to survive.
- [ ] Handle DPAPI failure honestly: if ciphertext cannot be decrypted (different Windows user, copied file), report "saved key could not be read on this computer" and offer Clear, instead of failing Shopping with a raw OS error.
- [ ] Follow the settings-compatibility rule for every new field: `#[serde(default)]`, `#[serde(alias = "...")]` for renamed keys, defaults applied only for missing data, no `deny_unknown_fields`, and an old `settings.json` must open unchanged.

### Settings UI (Tools section)

- [ ] Rework the AI block in `frontend/src/features/Settings/ToolsSection.tsx` into: provider picker, per-provider key password field with Set/Clear buttons and a "Saved on this computer" masked hint, endpoint field shown only for Custom/Local, and a model field with provider-fetched suggestions.
- [ ] Add a Test connection button that reports success or the plain-language provider error inline, without closing Settings.
- [ ] Keep the translate provider fields, `shoppingRegion`, and `shoppingModel` rows working, updating the shopping model hint to name the active provider.
- [ ] Show a one-line privacy note under the key field: encrypted with Windows DPAPI for this Windows user, never synced, never exported.

### Phase 0 tests and acceptance

- [ ] Add Rust unit tests for the OpenAI-compatible and Anthropic request bodies and response parsing, including Anthropic `content[]` text joining.
- [ ] Add a DPAPI round-trip test (encrypt then decrypt returns the original; ciphertext does not contain the plaintext).
- [ ] Add a settings migration test: a `settings.json` fixture with a plaintext `assistantKey` loads, migrates, and re-saves with no plaintext key anywhere in the file.
- [ ] Add a sync test proving a settings document written through `sync_conflicts` contains no key material.
- [ ] Manually verify with `pnpm tauri dev`: set an OpenAI key, test the connection, run Find deals and get a summary, clear the key, and confirm deals still appear without a summary; repeat the no-key path with the Local/Ollama default.

## Phase 1 — One Shopping Tool, One Popover, One Product Model

### Merge the two tools into one

- [ ] Keep a single entry in `BUILTIN_TOOLS` (`frontend/src/features/BuiltInExtensions/builtin.ts`): id `shopping`, name "Shopping", description covering both jobs, for example "Track prices and find coupons and better prices for this product." Remove the separate `price` entry.
- [ ] Keep one toolbar button (`ShoppingButton`) and one popover kind (`shopping` in `PopupKind` and the popup switch in `frontend/src/features/Popup/Popup.page.tsx`); delete the `price` button, popover component, and routing once the combined popover covers their features.
- [ ] Move the surviving UI into the shopping feature folder (`frontend/src/features/BuiltInExtensions/shopping/`), with the tracked-items section as its own small component file rather than growing one large popover file.
- [ ] Migrate saved toolbar state: a stored `price` id in `toolbarOrder` or `hiddenToolbarExtensions` maps to `shopping` on load (deduplicated if both are present), so nobody loses a pin or gains a duplicate button; unknown retired ids keep rendering nothing as today.
- [ ] Decide the address-bar behavior once: Shopping is one normal pinnable tool with no fixed slot; do not keep two icons for the same product.

### Combined popover

- [ ] Rebuild the Shopping popover in two stacked sections over the current product: **This product** (page price, Track/Untrack toggle, target state when tracked, sparkline when history exists) and **Deals** (Find deals, coupons with copy, cheaper offers), replacing the separate Price popover entirely.
- [ ] When the current page is already tracked, show its current drop, target price, and last-checked or error state at the top of the popover, with Check now for that item.
- [ ] When the current page is not a product page, show the tracked-items summary instead: top drops, target hits, items with errors, and an Open Shopping page button, so the tool is still useful off product pages.
- [ ] Keep the tracked list in the popover short (top few by drop) with an Open Shopping page button that opens `ce://shopping` in a tab; the full list lives on the page (Phase 3), not in the popover.
- [ ] Show target-hit and error badges in the popover so a failed background check is visible without opening the page.
- [ ] Stop swallowing list failures the way `Price.tsx` does today (`listPrices().catch(() => undefined)`); surface the small error text pattern used by the other popover states.

### Unified product identity

- [ ] Define one canonical product key used by tracking, deals, and coupon feedback: canonical URL first (tracking parameters stripped, scheme/host normalized), falling back to normalized product name + store host when no URL match exists.
- [ ] Dedupe tracking by that key: tracking the same product twice (from the page, from an offer, or by pasted URL) merges into the existing tracked item instead of inserting a duplicate.
- [ ] Make `find_deals`, the tracker, and the page all resolve "the current product" through the same helper, so the popover never shows deals for one product and tracking state for another.

### Data model v2 (`prices.json` stays the tracked-items store)

- [ ] Keep the existing `prices.json` file name and load path so every current tracked item survives; extend the item shape in `src-tauri/src/tools.rs` with `#[serde(default)]` on the file and item structs so a v1 file loads without any user action:
  - [ ] Stable string `id` generated when an item is first tracked (v1 items get one on migration).
  - [ ] Existing fields kept: `url`, `title`, `price`, `currency`, `availability`.
  - [ ] `imageUrl` from `og:image` or JSON-LD `image`, empty when the page has none.
  - [ ] `store` host derived from the URL, without a leading `www.`.
  - [ ] `trackedAt` and `lastCheckedAt` in milliseconds since 1970; zero means never.
  - [ ] `lastError` string, empty when the last check succeeded.
  - [ ] `targetPrice` (nullable) and alert toggles `alertOnDrop`, `alertBelowTarget`, `alertBackInStock`, each defaulting sensibly for migrated items.
  - [ ] `checkIntervalHours` with a sane default (for example 12) and a `paused` flag.
  - [ ] `note` and `tags` for the page UI, empty by default.
  - [ ] `history` array of `{ at, price, currency, availability }` points, seeded for v1 items from `previousPrice` then the current price.
- [ ] Cap `history` (for example the newest 500 points) and skip appending a point when price and availability are unchanged since the last point.
- [ ] On migration, derive dropped/stock-changed display state from `history` instead of the legacy one-shot booleans; keep the old fields readable until the first v2 save rewrites the file.
- [ ] Keep loading through `files::read_json` so a corrupt `prices.json` is still copied to `prices.json.bak` before defaults are used.
- [ ] Put the deals cache and coupon feedback in a separate `shopping.json` store (Phase 4) keyed by the same canonical product key; do not bolt deal data onto every price-history point.

### New and changed IPC

- [ ] Rename the price commands into the shopping surface while keeping old data readable: `untrack_price`, `check_price` (single item), `add_price_url`, and `update_tracked_price` (target, alerts, interval, paused, note, tags), each shipped as the usual three pieces — Rust command registered in `src-tauri/src/lib.rs`, camelCase type in `types/browser.ts`, wrapper in `frontend/src/lib/browser-client.ts`.
- [ ] Emit or return the full updated tracked list from mutating commands so the popover and the page re-render from one source, following the snapshot-event style used elsewhere.
- [ ] Keep `list_prices` / `track_price` / `check_prices` working until the combined popover and page have fully replaced their callers, then remove the dead wrappers rather than leaving two APIs for one job.

### Phase 1 tests and acceptance

- [ ] Add Rust tests for the v1-to-v2 migration (history seeding, defaults, id assignment) and canonical-URL dedupe.
- [ ] Add a settings test proving a saved toolbar order containing `price` (and one containing both `price` and `shopping`) normalizes to a single `shopping` entry.
- [ ] Manually verify with `pnpm tauri dev`: a browser that had Prices pinned shows one Shopping button in its place; tracking from the combined popover lists the item; existing tracked items from before the merge are all still there.

## Phase 2 — Extraction, Real Checking, Background Alerts

### Better price extraction

- [ ] Extend `parse_price` to walk JSON-LD `@graph` arrays and nested `Product` objects, not only top-level blocks.
- [ ] Read `AggregateOffer.lowPrice` (with `priceCurrency`) when a product only exposes an offer range, and prefer a concrete single `Offer` price when both exist.
- [ ] Read microdata: `itemprop="price"` `content` attributes and element text, plus `itemprop="availability"` for stock state.
- [ ] Add a DOM fallback in the page-read script: common price containers (`[data-testid*="price"]`, `[class*="price"]` near the title) parsed for a currency amount, used only when structured data found nothing.
- [ ] Capture the list (was) price when the page exposes one (`priceSpecification`, strikethrough list price) so the page can show "tracked below list."
- [ ] Capture the product image (`og:image` or JSON-LD `image`) at track time for the tracked-item cards.
- [ ] Keep every parser addition covered by an HTML fixture test next to the existing `parse_price` tests, including a page that must still return no price.

### Real price checking

- [ ] Add a direct-fetch checker: download the tracked URL with `ureq` (browser-like user agent, 15s timeout) and run the same parser used for open pages, so tracked items check correctly with no tab open.
- [ ] Fall back to reading the open tab through WebView2 when the direct fetch fails with 403, a captcha/challenge page, an empty body, or no parseable price, and record which method succeeded.
- [ ] Replace the silent `continue` in `check_prices` with a recorded per-item `lastError` and `lastCheckedAt`, so the UI can show exactly why an item is stale.
- [ ] Add single-item check and check-all with progress reporting to the page (checked count of total), a small concurrency limit for fetches (for example 4 at a time), and per-item failure isolation.
- [ ] Add manual Add by URL on the Shopping page: fetch and parse the pasted product URL, show the found title/price for confirmation, then track it without opening a tab.
- [ ] Keep the existing open-tab path working for pages that need the user's logged-in session, and prefer it when a matching tab is already loaded.

### Background checks and alerts

- [ ] Start a background worker on app launch, following the daily-check thread pattern in `src-tauri/src/update.rs`, that checks due items (interval elapsed, not paused) without blocking startup.
- [ ] Fire a Windows toast through the existing `system::toast` helper when an item drops in price, crosses at or below its `targetPrice`, or returns to stock, according to that item's alert toggles.
- [ ] Make each alert fire once per condition and re-arm only after the condition has cleared (price back above target, item out of stock again), so repeated checks do not spam notifications.
- [ ] Include the product title and new price in the toast text, and keep the toast launch behavior consistent with download toasts.
- [ ] Never alert on stale data: when both the fetch and the open-tab fallback fail, record the error and skip alerts for that round.

### Phase 2 tests and acceptance

- [ ] Add parser fixture tests for `@graph`, `AggregateOffer`, microdata, `og:image` capture, and a no-price page.
- [ ] Add Rust tests for history capping and alert re-arm logic.
- [ ] Manually verify with `pnpm tauri dev`: track a product, close its tab, run Check all and confirm the price updates; set a target above the current price and confirm one toast fires once; pause the item and confirm it is skipped; restart the app and confirm history and targets survived.

## Phase 3 — Shopping Page (`ce://shopping`)

- [ ] Create `frontend/src/features/Shopping/Shopping.page.tsx` that sets `document.title = "Shopping"`, registers a `case "shopping"` route in `frontend/src/main.tsx`, and follows the full-page pattern of `frontend/src/features/Boards/Boards.page.tsx` (appearance from settings, header, search, scrollable content, dialogs).
- [ ] Split the page into small components in the same feature folder (for example `TrackedGrid`, `TrackedTable`, `PriceSparkline`, `TargetDialog`, `ProductDetail`, `DealsPanel`) instead of one large file.
- [ ] Give every element an `id` as its first attribute and style with Tailwind, matching the existing page conventions.

### Tracked items (the price tracker, many items at once)

- [ ] Summary header showing: tracked count, how many dropped since first tracked, how many are at or below target, and total potential saving versus first-tracked prices.
- [ ] Card grid and compact table views (toggle persisted in component state) with thumbnail, title, store, current price, first-tracked price, change amount and percent, and last-checked or error state.
- [ ] Draw an inline SVG sparkline from `history` on each card/row, with min/max labels and a flat-line state for single-point histories; no chart library.
- [ ] Search by title, store, note, or tag; filters for dropped, below target, in stock, has error, and paused; sorting by biggest drop (amount and percent), lowest price, and most recently checked.
- [ ] Per-item actions: Open product (new tab), Check now, Set/edit target price and alert toggles, Pause/Resume checking, Edit note/tags, and Remove with a confirmation dialog.
- [ ] Bulk actions: Check all with progress, and Remove for selected items in table view with confirmation.
- [ ] Add by URL form (Phase 2 backend) with a confirmation step showing the parsed title and price before tracking.
- [ ] Empty state that explains tracking from a product page or Add by URL; loading skeletons while the first list loads; an error banner pattern like the Boards page for failed operations.
- [ ] Keyboard support consistent with other pages: Delete on selected rows and Escape to clear selection must work without trapping typing in inputs.

### Selected product detail (deals live beside the tracker)

- [ ] Selecting a tracked item (or arriving from the popover's Open Shopping page for the current product) shows a detail panel: large price history chart from `history`, target editor, and the latest deals for that product.
- [ ] The detail panel reuses the Phase 4 deals result for the product (coupons, verified offers, checked-at time) with Find deals / Search again, so deals are checked once and seen in both the popover and the page.
- [ ] Show cheaper verified offers inside the detail panel with Track buttons, so watching the same product at a second store is one click.
- [ ] Keep the detail panel a section of the one Shopping page; do not create a separate deals page or route.

### Phase 3 tests and acceptance

- [ ] Manually verify with `pnpm tauri dev`: track several products from different stores, open `ce://shopping`, and confirm all are visible at once with correct sparklines; filter to dropped items; edit a target; select an item and run Find deals from its detail panel; restart and confirm the view state and data survived.

## Phase 4 — Deals and Coupons

### Works without AI

- [ ] Split `find_deals` in `src-tauri/src/shopping.rs` into a search/extract stage and an optional AI summary stage, so a missing provider or key no longer fails the whole tool.
- [ ] Extract coupon codes from search results without AI using the existing verbatim grounding rules (code appears word-for-word in a result title or snippet, 3–30 characters, contains a digit or uppercase letter), credited to the result that shows it.
- [ ] Rank no-AI offers by parsed snippet price when present and label the section honestly as unverified until verification (below) runs.
- [ ] Return an `aiAvailable` flag (or equivalent) in `ShoppingDeals` so the UI can show results plus a one-line "Add an AI provider in Settings for summaries" hint instead of an error.
- [ ] When AI is available (Phase 0), keep the current JSON summary flow and grounding, unchanged in spirit.

### Verified offers

- [ ] Fetch each candidate offer URL (bounded count, short timeout, browser-like user agent) and re-run `parse_price` from Phase 2 to confirm the live price and availability before an offer is badged as verified.
- [ ] Drop offers whose page cannot be fetched or parsed, whose confirmed price is not below the current page price, or that redirect back to the current product page.
- [ ] Flag likely variant mismatches by product-name token overlap between the tracked product name and the offer page title (for example different size, color-only listing, or accessory), and show the flag instead of silently presenting them as the same product.
- [ ] Show shipping or "free shipping" hints only when the fetched page's structured data exposes them; never guess a total price.
- [ ] Keep the final offer list sorted cheapest-first by confirmed price, with the source store and a link, as today.
- [ ] Add Track beside each verified offer so a cheaper store becomes a tracked item in one click, using the offer URL and the Phase 1 canonical product key.

### Wider, cached search

- [ ] Add queries beyond the current two: `{name} coupon`, `{name} buy`, and a retailer-targeted query using the current store's competitors only when that list is configured; keep results merged and deduplicated by URL.
- [ ] Keep skipping ads in DuckDuckGo HTML results and keep the captcha detection error, with a friendlier retry hint.
- [ ] Respect `shoppingRegion` for every query and show prices in the page's currency where known.
- [ ] Cache the last result per product in the `shopping.json` store (canonical product key, results, timestamps) for about an hour; show "Checked Xm ago" with Search again forcing a fresh run, in both the popover and the page detail panel.
- [ ] Persist nothing about searches beyond that cache: no query history is written anywhere else.

### Coupon quality

- [ ] Store per-code local feedback in `shopping.json`: tried (timestamp), worked, and failed, set from small buttons on each coupon row; never auto-submit feedback anywhere.
- [ ] Show expiry hints when a snippet mentions a date or "expired", and sort codes with recent worked feedback first without hiding untested codes.
- [ ] Keep the disclaimer that codes and prices come from search results and may have expired.
- [ ] Keep the copy-code button and source link behavior from the current popover.

### Try at checkout (separate, later, off by default)

- [ ] Treat this as its own opt-in checkbox, not part of the first combined release: detect a coupon/promo field on a checkout-like page only after the user clicks Try codes.
- [ ] Apply codes one at a time through the page's own field and apply button, read the resulting total or discount line after each attempt, and restore the best code at the end; stop immediately on any payment, address, or login field interaction.
- [ ] Never store checkout page content, totals tied to an account, or entered codes outside the local feedback store.
- [ ] Require a fresh manual review of the safety rules before this checkbox is implemented.

### Phase 4 tests and acceptance

- [ ] Keep and extend the existing `shopping.rs` tests: invented codes dropped, unknown offer URLs dropped, ads skipped, wrapped DuckDuckGo links resolved.
- [ ] Add tests for the no-AI extraction path (codes and priced offers from fixture search results with no provider configured) and for variant-mismatch flagging.
- [ ] Add a test that a verified offer whose fetched price is not actually lower is removed.
- [ ] Manually verify with `pnpm tauri dev`: run Find deals with no AI configured and confirm codes still appear; configure a provider and confirm the summary returns; track the product from the popover and see the same item, with its deals, on `ce://shopping`; mark a code as tried and confirm the state survives a restart.

## Cross-Cutting Rules for Every Phase

- [ ] One extension means one of everything user-facing: one `BUILTIN_TOOLS` entry, one toolbar button, one `PopupKind`, one page route. Retired ids (`price`) are migrated in saved settings, never left as a second working tool.
- [ ] Every new IPC capability ships all three pieces together: Rust command registered in `generate_handler!` in `src-tauri/src/lib.rs`, camelCase type in `types/browser.ts`, and a typed wrapper in `frontend/src/lib/browser-client.ts`; components never call `invoke` directly.
- [ ] Structs crossing the bridge use `#[serde(rename_all = "camelCase")]` and commands return `Result<T, String>` with user-facing error text.
- [ ] Every new persisted field uses `#[serde(default)]` plus a `Default` value; renamed fields keep an alias; retired or unknown values fall back in `normalize`; no store gains `deny_unknown_fields`.
- [ ] Loading an already-current `settings.json`, `prices.json`, or `shopping.json` changes nothing; the next save writes the current shape; a file that fails to parse is copied to `<name>.json.bak` by `files::read_json` before defaults are used.
- [ ] API keys and checkout data never enter the sync folder, profile archives, logs, or error messages; page and search text sent to an AI provider is sent only when the user runs Find deals.
- [ ] Anything that must overlap page content opens through the existing popup system; the full tool (`ce://shopping`) is a tab, and the popover stays a quick view that links to it.
- [ ] Every HTML element gets an `id` as its first attribute, styling is Tailwind, and classes are merged with `cn()` where conditional.
- [ ] Split large UI work into small files per feature folder (page, cards/rows, dialogs, hooks) rather than growing one component file.
- [ ] Long network work runs off the UI path (`blocking` / worker threads as in existing commands), and WebView2 access stays inside `with_webview` on the main thread.
- [ ] State-changing flows that the user may regret (Remove tracked item, Clear API key) use a confirmation or an undo toast consistent with existing patterns.

## Verification Checklist (per phase, before checking off)

- [ ] Run the Rust tests for the touched modules (`cargo test` in `src-tauri`) and add fixtures beside the existing tests in `tools.rs` and `shopping.rs` rather than a new test harness.
- [ ] Run the frontend build/typecheck with `pnpm build` from the repo root and fix every TypeScript error introduced.
- [ ] Verify in the native app with `pnpm tauri dev` (the Vite URL alone cannot exercise `invoke`, events, popups, or toasts).
- [ ] Test with a pre-change data folder: copy the current `%APPDATA%\com.ce.browser` data (or selected sync directory), launch the new build, and confirm settings, toolbar pins, and tracked prices open intact.
- [ ] Confirm a corrupt copy of each touched JSON file produces a `.bak` and a working default state instead of a crash or an empty-window failure.

## Suggested Order and Dependencies

- [ ] Phase 0 (providers + DPAPI keys) first if AI summaries matter, because Find deals' summary path depends on the provider dispatch; the rest of Shopping does not wait for it.
- [ ] Phase 1 (one tool, one popover, product model v2) next, because the page, checking, and deals integration all read the unified tracked-item shape.
- [ ] Phase 2 (extraction, checking, alerts) after Phase 1, giving the page real data to show.
- [ ] Phase 3 (the `ce://shopping` page) after Phase 2, with the tracker section first and the detail panel once deals caching exists.
- [ ] Phase 4 (deals and coupons) last, with the no-AI fallback landing before verified offers; try-at-checkout stays optional and behind its own review.

## Explicit Non-Goals

- [ ] No separate Prices tool, popover, page, or toolbar pin remains when this plan is done; price tracking is a section of Shopping, not its own extension.
- [ ] No Assistant or general page-chat feature is part of this plan; AI is used only for Shopping summaries and ranking.
- [ ] No cloud sync of API keys, price history, or shopping feedback.
- [ ] No price checking while the app is closed; there is no Windows service or scheduled task.
- [ ] No non-Windows key storage design; DPAPI is Windows-only, matching the app.
- [ ] No additional search engines beyond DuckDuckGo, and no scraping of retailer search pages as a coupon source.
- [ ] No automatic purchases, cart changes, or checkout submissions; try-at-checkout only fills a promo field when explicitly invoked and reviewed.
