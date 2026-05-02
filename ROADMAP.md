# Roadmap

Future development plan for iiif-rs. Contributions welcome for any item.

The plan is structured **correctness first, features second**. v0.2.1 closes spec-compliance gaps and security footguns discovered in the v0.2.0 audit. Only after that does new functionality land.

## v0.2.1 — Correctness & Spec Compliance

Punch list of audit findings against the IIIF specs in `specyfikacja/`. Each item references the offending file:line.

### Authorization Flow API 2.0
- [x] **Probe service must return HTTP 200** — auth state lives in the JSON `status` field, not the HTTP status.
- [x] **Token success body must carry `type: "AuthAccessToken2"`**.
- [x] **Token error body must carry `type: "AuthAccessTokenError2"`** with `profile` from the spec enum (`invalidRequest | invalidOrigin | missingAspect | invalidAspect | expiredAspect | unavailable`).
- [x] **Reject `targetOrigin = "*"` fallback for success** — when `?origin=` is missing, return `AuthAccessTokenError2 { profile: "invalidOrigin" }`. Error bodies still go to `"*"` since they carry no token.
- [x] **`ProbeResult` JSON carries `@context`**. (`location` and `substitute[]` deferred to v0.3.0 tiered-access work.)
- [x] **Advertise `AuthProbeService2` in resources** — embedded in `service[]` of both info.json and Manifest image body, with `http://iiif.io/api/auth/2/context.json` prepended to the parent context array.
- [x] **Drop spurious `profile` field** from token/probe/logout sub-services.
- [x] **`Set-Cookie` security attributes** — `Secure; SameSite=None` over HTTPS (so cross-site iframe token flow works), `HttpOnly; SameSite=Lax` over HTTP for local dev.

### Content Search API 2.0
- [x] **Pagination** — emits `partOf: AnnotationCollection { total, first, last }`, `next`, `prev`, `startIndex`; page size 50; query param `?page=N`.
- [x] **`motivation` is a valid v2 query param** — no longer reported in `ignored[]` for autocomplete.
- [x] **Stable annotation IDs** — passes through `IndexedAnnotation.id` rather than per-response index.
- [x] **Replace ad-hoc `urlencoded`** with `percent_encoding` crate.

### Change Discovery API 1.0
- [x] **Activity model** — `target` (required for `Move`), `startTime` (required for `Refresh`), optional `actor`, `summary`. New helpers `record_move()` and `record_refresh()`.
- [x] **Absolute IRI for Activity `id`** — `ActivityStore::new` now requires `base_url`; activity IDs are minted as `{base_url}/activity/{n}`.
- [x] **Replace hand-rolled ISO 8601 formatter** with `chrono`.
- [x] **Empty store omits `first` and `last`** — `OrderedCollection::new(_, 0, 0)` returns `totalItems: 0` with no page references.

### Image API 3.0
- [x] **Derive `extraFeatures` from `ImageConfig`** — `sizeUpscaling` omitted when `allow_upscaling=false`; `jsonldMediaType` added.
- [x] **Canonical Link emitted only with real source dimensions** — passes `Option<(w,h)>` to `build_image_response`; cache hits skip the header rather than emit `0,0` defaults.
- [x] **Stable hashing** — `compute_etag` and `disk_cache_path` now use SHA-256 (truncated to 64 bits) instead of `DefaultHasher`. Deterministic across builds, Rust versions, and platforms.
- [x] **Disk-cache I/O off the runtime** — `std::fs::read`/`create_dir_all`/`write` wrapped in `spawn_blocking`.

### Identifier handling
- [x] **UTF-8 percent-decoding fixed** — decoder now buffers bytes and uses `String::from_utf8`. Tests added for `%C3%A9` (é), `%E2%9C%93` (✓), Polish diacritics (`%C5%82`), the `%2525` → `%25` double-encoding case, and rejection of invalid UTF-8 sequences.

### Server hygiene
- [x] **Remove demo seeds** — `seed_search_index` and `seed_activities` ran on every boot, polluting any future persistent store (`crates/iiif-server/src/main.rs:353-381`).
- [x] **Fail-fast when `protected_dirs` is non-empty but `auth.enabled = false`** — silent exposure is worse than refusing to start (`crates/iiif-server/src/main.rs:115-134`).
- [x] **Graceful shutdown for HTTPS** — TLS branch now spawns a SIGINT handler that calls `axum_server::Handle::graceful_shutdown(Some(30s))`.
- [x] **Validate TLS cert/key pairing** — fail-fast at startup when only one of `tls_cert`/`tls_key` is set.

### Test coverage
- [x] **Integration tests for `iiif-image` handlers** — 303 base-URI redirect, identifier validation (path traversal, invalid UTF-8), 400 on bad region. (304/Canonical-Link tests deferred to v0.3.0 — they need a real source-image fixture.)
- [x] **End-to-end tests for auth flow** — probe HTTP 200 (with and without Bearer), token `AuthAccessToken2` shape, `AuthAccessTokenError2` profiles (`invalidOrigin`, `missingAspect`), `messageId` round-trip, `targetOrigin` enforcement, `Set-Cookie` security attributes, logout invalidation.
- [x] **Search paging tests** — `partOf.first/last/total`, `next`/`prev`, `startIndex` covered by `search_response_paging_links`. Hit augmentation deferred to v0.3.0.
- [x] **Identifier UTF-8 / double-percent-encoding tests** in `iiif-core::identifier` (`%C3%A9`, `%E2%9C%93`, Polish diacritics, `%2525` → `%25`).
- [x] **Tests for fail-fast security config** — `iiif_core::config::validate_security_config` extracted and unit-tested for `protected_dirs` without `auth.enabled`, half-set TLS pair, and the OK paths.
- [x] **XSS regression tests for `/auth/token`** — `</script>` breakout via `?origin=` and via `?messageId=`. Strict origin validator + `</` → `<\/` JSON neutralisation.

## v0.3.0a — Foundations Refactor (done)

Compile-time type safety across the workspace. Invisible to IIIF clients but unblocks every later feature.

- [x] **`IiifError: IntoResponse`** in `iiif-core` — eliminated 4 wrapper structs (`PresentationError`, `DiscoveryError`, `StateError`, `ApiError`) and 4 ad-hoc `match status_u16` blocks. Handlers now return `Result<Response, IiifError>` directly with a uniform JSON error body.
- [x] **`iiif_core::services::Service` tagged enum** — replaces scattered `serde_json::Value` blobs for service descriptors. Variants: `ImageService3`, `AuthProbeService2`, `AuthAccessService2`, `AuthAccessTokenService2`, `AuthLogoutService2`, `SearchService2`, `AutoCompleteService2`. Each crate produces typed values via factory functions; `iiif-core` hosts the data types without depending on any other crate.
- [x] **Async `ImageStorage` trait** — `read_image`, `last_modified`, `exists`, `resolve_path` are now async (via `async-trait`). `containing_directory` stays sync (cheap in-memory lookup). Filesystem impl uses `tokio::fs`. All `tokio::task::spawn_blocking { storage.X(...) }` boilerplate gone from handlers; CPU work (image decode/process) still on the blocking pool.
- [x] **`AppState` typed (drop `Arc<dyn Any>`)** — minimal `AppState { config, storage }`. Optional services (`AuthStore`, `SearchIndex`, `ActivityStore`, `ImageCache`) wired via `axum::Extension<Arc<T>>`. Zero runtime downcasts. `iiif-server/main.rs` gates router merges + extension layers behind config (`if let Some(auth_store) = ... { app.merge(auth::router()).layer(Extension(auth_store)) }`).

## v0.3.0b — Spec Features

After v0.2.1 the implementation matches the literal spec. v0.3.0b fills in major spec features that v0.2.0 skipped entirely.

### Search — hit augmentation
- [ ] Sibling `annotations: [AnnotationPage]` with motivations `contextualizing` and `highlighting`.
- [ ] `target` as `SpecificResource` with `selector: TextQuoteSelector { prefix, exact, suffix }`.
- [ ] Multi-target arrays for phrase matches spanning annotations.
- [ ] ISO 8601 date range parsing for `date` parameter; OR-semantics for `motivation`/`user`.

### Auth — second and third patterns
- [ ] **`kiosk` pattern** — descriptor with `id`, no UI in opened tab.
- [ ] **`external` pattern** — descriptor without `id`/`label`; ambient auth (IP, prior SSO).
- [ ] **`AuthLogoutService2`** — endpoint that actively purges cookies AND token map (today `cleanup()` exists at `crates/iiif-auth/src/store.rs:101` but no scheduler invokes it).
- [ ] **Tiered access** — populate `substitute[]` in probe response (low-res image when access denied).
- [ ] **Origin allowlist** — validate `?origin=` against config-driven whitelist on access AND token services.

### Presentation — model gaps
- [ ] Add `AnnotationCollection`, `placeholderCanvas`, `accompanyingCanvas` types and fields.
- [ ] Add `SpecificResource` + selectors (`FragmentSelector`, `PointSelector`, `SvgSelector`).
- [ ] Refactor `Service` into a `#[serde(tag="type")]` enum (`ImageService3`, `AuthAccessService2`, `SearchService2`, `AutoCompleteService2`, ...). Today it's one flat struct conflating all variants.
- [ ] Type the untyped `start: serde_json::Value` and `Range.supplementary` as proper structs.
- [ ] **Register routes for `/canvas/{id}`, `/annotation-page/{id}`, `/annotation/{id}`, `/range/{id}`** — currently the builder mints these URIs but they 404 when dereferenced.
- [ ] **Sidecar metadata** — read `images/<name>.json` or `images/<name>.toml` and merge into Manifest (`label`, `metadata[]`, `summary`, `rights`, `provider`). Today every Manifest has only the filename stem as label.
- [ ] **Content negotiation** — honor `Accept: application/json` (no profile parameter), return 406 for unacceptable Accept (`crates/iiif-presentation/src/handlers.rs:28-60`).


## v0.4.0 — Storage Backends

- [ ] **S3 / MinIO storage** — read images from Amazon S3 or S3-compatible object storage.
- [ ] **Azure Blob Storage** — Azure cloud storage backend.
- [ ] **Google Cloud Storage** — GCS bucket backend.
- [ ] **HTTP remote storage** — fetch source images from remote HTTP URLs with local caching.
- [ ] **Multi-source routing** — route different identifier prefixes to different storage backends.

## v0.5.0 — Auth & Multi-tenancy

- [ ] **OAuth2 / OpenID Connect** — external identity provider integration (Keycloak, Auth0, Google).
- [ ] **LDAP / Active Directory** — institutional authentication for universities and museums.
- [ ] **Role-based access control** — roles (viewer, editor, admin) with per-collection permissions.
- [ ] **API keys** — token-based access for machine-to-machine integrations.
- [ ] **Multi-tenant mode** — multiple institutions sharing one server with isolated storage and auth.

## v0.6.0 — Presentation API Enhancements

- [ ] **Manifest editor API** — CRUD endpoints for creating/editing Manifests, Canvases, and Annotations.
- [ ] **Database-backed manifests** — store manifests in PostgreSQL/SQLite instead of auto-generation.
- [ ] **IIIF Cookbook recipes** — implement common patterns: multi-page books, newspapers, audio/video, maps.
- [ ] **OCR integration** — auto-generate text annotations from images using Tesseract or cloud OCR.
- [ ] **Annotation storage** — persistent annotation store with W3C Web Annotation Protocol support.

## v0.7.0 — Search & Discovery at Scale

- [ ] **Tantivy full-text search** — replace in-memory index with tantivy for persistent, scalable full-text search.
- [ ] **Elasticsearch / OpenSearch** — optional external search backend for large collections.
- [ ] **Faceted search** — filter by date, language, type, collection.
- [ ] **Change Discovery webhooks** — push notifications on resource changes (WebSub).
- [ ] **OAI-PMH compatibility** — bridge between Change Discovery API and OAI-PMH harvesters.

## v0.8.0 — Media Support

- [ ] **JPEG 2000 (JP2)** — native JP2 decode/encode for archival-quality images.
- [ ] **IIIF AV (audio/video)** — time-based media support in Presentation API with HLS/DASH streaming.
- [ ] **PDF generation** — on-the-fly PDF rendering from Manifests.
- [ ] **3D model support** — IIIF 3D extension for museum objects (glTF/USDZ).
- [ ] **libvips integration** — currently blocked: Rust bindings (`libvips` and `libvips-rs` crates) incompatible with Rust 1.94+. Benchmarked at 6-27× faster. Will integrate when crates are updated.

## v1.0.0 — Stable Release

- [ ] **Comprehensive rustdoc** — documentation for every public API with examples.
- [ ] **Integration test suite** — end-to-end tests for all 6 APIs running against a live server.
- [ ] **Benchmark suite** — automated performance benchmarks comparing with Cantaloupe and IIPImage.
- [ ] **Deployment guides** — production deployment docs for Nginx, Caddy, Kubernetes, AWS ECS.
- [ ] **Plugin system** — trait-based hooks for custom auth, storage, processing, and metadata providers.
- [ ] **Stable API guarantee** — SemVer stability commitment for the public Rust API.

## Long-term Vision

- [ ] **IIIF v4 support** — track and implement upcoming IIIF specification versions.
- [ ] **WebAssembly client** — WASM build for client-side IIIF processing in the browser.
- [ ] **Federated search** — cross-server search across multiple IIIF endpoints.
- [ ] **AI-powered annotations** — auto-generate descriptions, tags, and regions using vision models.
- [ ] **crates.io publishing** — publish individual crates for embedding IIIF in other Rust applications.
