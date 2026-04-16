# Roadmap

Future development plan for iiif-rs. Contributions welcome for any item.

## v0.2.0 — Performance & Production Readiness

- [ ] **libvips integration** — Replace `image` crate with libvips bindings for 5-10x faster image processing, lower memory usage, and streaming pipeline
- [ ] **Response caching** — In-memory LRU cache (moka) for processed images; cache key = request URI + file mtime
- [ ] **Disk tile cache** — Pre-generate and cache tiles on disk for frequently accessed images; serve directly without re-processing
- [ ] **HTTP/2 + TLS** — Native HTTPS support via rustls; HTTP/2 for concurrent tile loading
- [ ] **Rate limiting** — Per-IP request throttling via tower middleware to prevent abuse
- [ ] **Request timeout** — Configurable timeout for image processing to prevent resource exhaustion
- [ ] **Prometheus metrics** — `/metrics` endpoint with request count, latency histograms, cache hit rate, active connections

## v0.3.0 — Storage Backends

- [ ] **S3 / MinIO storage** — Read images from Amazon S3 or S3-compatible object storage
- [ ] **Azure Blob Storage** — Support for Azure cloud storage
- [ ] **Google Cloud Storage** — Support for GCS buckets
- [ ] **HTTP remote storage** — Fetch source images from remote HTTP URLs with local caching
- [ ] **Multi-source routing** — Route different identifier prefixes to different storage backends

## v0.4.0 — Auth & Multi-tenancy

- [ ] **OAuth2 / OpenID Connect** — External identity provider integration (Keycloak, Auth0, Google)
- [ ] **LDAP / Active Directory** — Institutional authentication for universities and museums
- [ ] **Role-based access control** — Roles (viewer, editor, admin) with per-collection permissions
- [ ] **API keys** — Token-based access for machine-to-machine integrations
- [ ] **Multi-tenant mode** — Multiple institutions sharing one server with isolated storage and auth

## v0.5.0 — Presentation API Enhancements

- [ ] **Manifest editor API** — CRUD endpoints for creating/editing Manifests, Canvases, and Annotations
- [ ] **Database-backed manifests** — Store manifests in PostgreSQL/SQLite instead of auto-generation
- [ ] **IIIF Cookbook recipes** — Implement common patterns: multi-page books, newspapers, audio/video, maps
- [ ] **OCR integration** — Auto-generate text annotations from images using Tesseract or cloud OCR
- [ ] **Annotation storage** — Persistent annotation store with W3C Web Annotation Protocol support

## v0.6.0 — Search & Discovery

- [ ] **Tantivy full-text search** — Replace in-memory index with tantivy for persistent, scalable full-text search
- [ ] **Elasticsearch/OpenSearch** — Optional external search backend for large collections
- [ ] **Faceted search** — Filter by date, language, type, collection
- [ ] **Change Discovery webhooks** — Push notifications on resource changes (WebSub)
- [ ] **OAI-PMH compatibility** — Bridge between Change Discovery API and OAI-PMH harvesters

## v0.7.0 — Media Support

- [ ] **JPEG 2000 (JP2)** — Native JP2 decode/encode for archival-quality images
- [ ] **IIIF AV (audio/video)** — Time-based media support in Presentation API with HLS/DASH streaming
- [ ] **PDF generation** — On-the-fly PDF rendering from Manifests
- [ ] **3D model support** — IIIF 3D extension for museum objects (glTF/USDZ)

## v1.0.0 — Stable Release

- [ ] **Comprehensive rustdoc** — Documentation for every public API with examples
- [ ] **Integration test suite** — End-to-end tests for all 6 APIs running against live server
- [ ] **Benchmark suite** — Automated performance benchmarks comparing with Cantaloupe, IIPImage
- [ ] **Deployment guides** — Production deployment docs for Nginx, Caddy, Kubernetes, AWS ECS
- [ ] **Plugin system** — Trait-based hooks for custom auth, storage, processing, and metadata providers
- [ ] **Stable API guarantee** — SemVer stability commitment for public Rust API

## Long-term Vision

- [ ] **IIIF v4 support** — Track and implement upcoming IIIF specification versions
- [ ] **WebAssembly client** — WASM build for client-side IIIF processing in the browser
- [ ] **Federated search** — Cross-server search across multiple IIIF endpoints
- [ ] **AI-powered annotations** — Auto-generate descriptions, tags, and regions using vision models
- [ ] **crates.io publishing** — Publish individual crates for embedding IIIF in other Rust applications
