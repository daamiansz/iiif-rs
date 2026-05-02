# IIIF Server

A complete [IIIF](https://iiif.io/) (International Image Interoperability Framework) server written in Rust, implementing all 6 IIIF specifications.

[![CI](https://github.com/daamiansz/iiif-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/daamiansz/iiif-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org/)

## Features

| Specification | Version | Status |
|---|---|---|
| [Image API](https://iiif.io/api/image/3.0/) | 3.0 | Level 2 compliance; runtime-derived `extraFeatures`; UTF-8 identifiers |
| [Presentation API](https://iiif.io/api/presentation/3.0/) | 3.0 | Manifest + Collection + dereferenceable Canvas/AnnotationPage/Annotation; typed Selectors, AnnotationCollection, placeholderCanvas, content negotiation (406 on unsupported Accept) |
| [Authorization Flow API](https://iiif.io/api/auth/2.0/) | 2.0 | All three patterns (`active` / `kiosk` / `external`); spec-compliant token bodies; probe always HTTP 200; tiered access with `substitute[]`; origin allowlist; XSS-hardened postMessage; logout actively purges tokens |
| [Content Search API](https://iiif.io/api/search/2.0/) | 2.0 | Paginated AnnotationPage with `partOf.AnnotationCollection`; hit augmentation via `TextQuoteSelector`; OR-semantics `motivation`; ISO 8601 date range parsing; autocomplete |
| [Content State API](https://iiif.io/api/content-state/1.0/) | 1.0 | Encode/decode/validate |
| [Change Discovery API](https://iiif.io/api/discovery/1.0/) | 1.0 | OrderedCollection + Activity (Create/Update/Delete/Move/Refresh) |

### Highlights

- **Fast** — async I/O (tokio), zero-copy image pipeline, Lanczos3 resampling
- **Safe** — Rust memory safety, no `unsafe`, input validation on all boundaries
- **Lightweight** — 13 MB release binary, ~50 MB Docker image
- **Configurable** — TOML file, environment variables, or both
- **Observable** — structured logging (tracing), ETag/Last-Modified caching, health checks

## Quick Start

### Option 1: Docker Compose

```bash
# Clone the repository
git clone https://github.com/daamiansz/iiif-rs.git
cd iiif-rs

# Add images (see Image Directory Structure below)
cp /path/to/your/images/*.jpg images/

# Start the server
docker compose up -d

# Open http://localhost:8080/{image_name}/info.json
```

### Option 2: From Source

```bash
# Prerequisites: Rust 1.94+
cargo build --release

# Create an images directory and add some images
mkdir -p images
cp /path/to/your/images/*.jpg images/

# Run the server
./target/release/iiif-server
```

The server starts at `http://localhost:8080`.

## API Endpoints

### Image API 3.0

```
GET /{identifier}/info.json
GET /{identifier}/{region}/{size}/{rotation}/{quality}.{format}
```

Example: `http://localhost:8080/painting/full/800,/0/default.jpg`

| Parameter | Values |
|---|---|
| region | `full`, `square`, `x,y,w,h`, `pct:x,y,w,h` |
| size | `max`, `w,`, `,h`, `w,h`, `!w,h`, `pct:n` (prefix `^` for upscaling) |
| rotation | `0`-`360` (prefix `!` for mirroring) |
| quality | `default`, `color`, `gray`, `bitonal` |
| format | `jpg`, `png`, `webp`, `gif`, `tif` |

### Presentation API 3.0

```
GET /manifest/{identifier}          — Manifest for an image
GET /collection/top                 — Root collection of all images
GET /canvas/{identifier}/p1         — Standalone Canvas (with @context)
GET /annotation-page/{identifier}/p1 — Standalone AnnotationPage
GET /annotation/{identifier}/p1-image — Standalone Annotation
GET /range/{identifier}/{rid}       — Standalone Range (404 unless persisted)
```

Content negotiation: `Accept: application/ld+json` (default, includes profile parameter), `Accept: application/json` (no profile), `Accept: text/plain` → 406. `Vary: accept` is emitted on every response.

For protected images the Manifest body's `service[]` includes an `AuthProbeService2` descriptor with the full hierarchy `probe → access → [token, logout]`, and the auth API context is prepended to `@context`.

### Authorization Flow API 2.0

```
GET  /auth/login                    — Login page
POST /auth/login                    — Submit credentials
GET  /auth/token                    — Token service (iframe/postMessage)
GET  /auth/probe/{resource_id}      — Probe service (always HTTP 200; status in body)
GET  /auth/logout                   — Clear session
```

The probe response shape is `{ "@context", "type": "AuthProbeResult2", "status": 200|401|... }`. The token service emits `AuthAccessToken2` (success) or `AuthAccessTokenError2` with one of the spec profile values (`invalidOrigin`, `missingAspect`, `invalidAspect`, `expiredAspect`, `invalidRequest`, `unavailable`). `Set-Cookie` carries `Secure; SameSite=None` over HTTPS, `HttpOnly; SameSite=Lax` over HTTP.

### Content Search API 2.0

```
GET /search?q={terms}&motivation={m1 m2}&date={iso8601-range}&user={u}&page=N
GET /autocomplete?q={prefix}&motivation={m}&min={n}
```

| Parameter | Notes |
|---|---|
| `q` | Space-separated terms; AND across terms |
| `motivation` | Space-separated motivations; OR across values |
| `date` | `YYYY-MM-DDThh:mm:ssZ/YYYY-MM-DDThh:mm:ssZ` (UTC `Z` mandatory); 400 on malformed |
| `user` | Space-separated URIs; recognised, in-memory backend cannot honour |
| `page` | Zero-based page index; page size 50 |

The response is a paginated `AnnotationPage` with `partOf: AnnotationCollection { total, first, last }`, `next`/`prev`, and `startIndex`. A sibling `annotations[]` array carries hit augmentation: each hit Annotation has `motivation: "contextualizing"` and a `target` SpecificResource pinning the matched body via `TextQuoteSelector { prefix, exact, suffix }`. Hit IDs are stable across queries (SHA-256 of `source|term|position`).

### Content State API 1.0

```
POST /content-state/encode          — Encode JSON to base64url
GET  /content-state/decode?content= — Decode base64url to JSON
POST /content-state                 — Validate + encode
```

### Change Discovery API 1.0

```
GET /activity/all-changes           — OrderedCollection
GET /activity/page/{n}              — OrderedCollectionPage
```

## Configuration

Configuration is loaded from `config.toml` (or the path in `IIIF_CONFIG`). Every setting can be overridden with environment variables.

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `IIIF_CONFIG` | `config.toml` | Path to TOML config file |
| `IIIF_HOST` | `127.0.0.1` | Bind address |
| `IIIF_PORT` | `8080` | Bind port |
| `IIIF_BASE_URL` | `http://localhost:8080` | Public base URL |
| `IIIF_STORAGE_PATH` | `./images` | Images directory |
| `IIIF_MAX_WIDTH` | `4096` | Max output width (px) |
| `IIIF_MAX_HEIGHT` | `4096` | Max output height (px) |
| `IIIF_MAX_AREA` | `16777216` | Max output area (px) |
| `IIIF_ALLOW_UPSCALING` | `true` | Allow `^` upscaling |
| `IIIF_TILE_WIDTH` | `512` | Tile width for info.json |
| `IIIF_AUTH_ENABLED` | `false` | Enable auth flow |
| `IIIF_AUTH_COOKIE` | `iiif_access` | Session cookie name |
| `IIIF_AUTH_TOKEN_TTL` | `3600` | Token lifetime (seconds) |

### Example config.toml

```toml
[server]
host = "127.0.0.1"
port = 8080
base_url = "http://localhost:8080"

[image]
max_width = 4096
max_height = 4096
max_area = 16777216
allow_upscaling = true
tile_width = 512
tile_scale_factors = [1, 2, 4, 8, 16]

[storage]
root_path = "./images"

[auth]
enabled = false
pattern = "active"            # "active" | "kiosk" | "external"
cookie_name = "iiif_access"
token_ttl = 3600
protected_dirs = ["restricted"]
allowed_origins = []          # empty = any well-formed origin; non-empty = whitelist
token_sweep_interval_secs = 300  # 0 = disable background token cleanup
substitute_size = ""          # IIIF Image API size param for the degraded preview
                              # served when access is denied (e.g. "^200,").
                              # Empty = no substitute resource in probe response.

[[auth.users]]
username = "admin"
password = "changeme"
```

## Storage backends

By default the server reads images from `[storage].root_path` on local disk. v0.4.0 adds optional cloud and remote-HTTP sources via the `[[storage.sources]]` array, routed across multiple backends in declaration order with the filesystem catch-all appended last.

```toml
[storage]
root_path = "./images"  # always present, used as the catch-all source

# S3 (or any S3-compatible — MinIO, Wasabi, R2, ...)
[[storage.sources]]
kind          = "s3"
label         = "rare-books"
bucket        = "iiif-rare"
region        = "eu-west-1"
prefix        = "manuscripts/"      # object-key prefix inside the bucket
access_zone   = "restricted"        # surfaces in `access_zone()` for auth
prefix_filter = "rare-"             # only ids starting with "rare-" hit this source

# Azure Blob Storage
[[storage.sources]]
kind        = "azure"
label       = "azure-archive"
account     = "myaccount"
container   = "iiif"
prefix      = "originals/"

# Google Cloud Storage
[[storage.sources]]
kind   = "gcs"
label  = "gcs-thumbs"
bucket = "my-iiif-bucket"

# HTTP remote (read-only fetch from any HTTP-accessible bucket / mirror)
[[storage.sources]]
kind          = "http"
label         = "wikimedia"
url           = "https://upload.wikimedia.org/wikipedia/commons/"
prefix_filter = "wm-"
```

Routing rules:

- Sources are tried **in declaration order**. The first one whose `prefix_filter` matches the identifier (or which has no filter) handles the request.
- `prefix_filter` keeps cold requests fast: a 3-source cloud setup without filters would HEAD all three sources before falling back to filesystem.
- `access_zone` integrates with the existing `auth.protected_dirs` model — set `access_zone = "restricted"` on a source and add `"restricted"` to `protected_dirs` to require login for everything that source serves.
- Identifiers containing `/` (e.g. `ark:/12025/654xz321`) become hierarchical keys in the cloud backend (`prefix/ark:/12025/654xz321.jpg`); no special encoding required.
- HTTP source fetches are cached on disk under `<tile_cache_dir>/source/` (SHA-256 keyed by source label + identifier). Delete the directory to refresh.

Credentials use the standard provider chains: `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` for S3, `AZURE_STORAGE_ACCOUNT_KEY` for Azure, `GOOGLE_APPLICATION_CREDENTIALS` for GCS.

## Image Directory Structure

The server scans `images/` and its immediate subdirectories. The image filename (without extension) becomes the identifier in URLs.

```
images/
├── painting.jpg              → http://localhost:8080/painting/info.json
├── photo.png                 → http://localhost:8080/photo/info.json
├── public/
│   └── landscape.jpg         → http://localhost:8080/landscape/info.json
└── restricted/
    ├── manuscript.jpg         → http://localhost:8080/manuscript/info.json
    └── private_letter.jpg     → http://localhost:8080/private_letter/info.json
```

- Images in the **root** and any **non-protected** subdirectory are publicly accessible
- Images in subdirectories listed in `protected_dirs` require authentication
- The subdirectory name does **not** appear in the URL — only the filename matters
- Supported formats: JPEG, PNG, WebP, GIF, TIFF

## Access Control

Protection is directory-based. Enable auth and list which subdirectories require login:

```toml
[auth]
enabled = true
protected_dirs = ["restricted"]

[[auth.users]]
username = "admin"
password = "changeme"
```

| Image location | URL | Without login | After login |
|---|---|---|---|
| `images/painting.jpg` | `/painting/info.json` | 200 | 200 |
| `images/public/landscape.jpg` | `/landscape/info.json` | 200 | 200 |
| `images/restricted/manuscript.jpg` | `/manuscript/info.json` | 401 | 200 |

No naming conventions or patterns — just move the file to the right folder.

### Auth interaction patterns

Set `auth.pattern` to one of:

- **`active`** — full UI flow: user clicks "Login", credentials form opens in a new tab, postMessage delivers the token. Default. Works when third-party cookies are allowed.
- **`kiosk`** — managed device: access service descriptor carries no UI strings. Same flow as active but the opened tab is expected to log in automatically (e.g. IP-restricted gallery kiosk).
- **`external`** — ambient auth (IP allowlist, prior SSO): the access service descriptor omits `id` entirely; clients skip the login tab and go straight to the token service.

The descriptor shape changes accordingly. The probe service URL and the token/logout sub-services stay the same in `active` and `kiosk`; `external` has no logout sub-service.

### Tiered access (degraded substitute)

When `auth.substitute_size = "^200,"` (or any IIIF size parameter), denied probes (`status: 401`) carry a `substitute[]` array pointing at a low-resolution version of the same image:

```json
{
  "@context": "http://iiif.io/api/auth/2/context.json",
  "type": "AuthProbeResult2",
  "status": 401,
  "substitute": [{
    "id": "http://localhost:8080/secret/full/^200,/0/default.jpg",
    "type": "Image", "format": "image/jpeg",
    "label": {"en": ["Low-resolution preview"]}
  }]
}
```

The middleware exempts requests for that exact size from the auth check, so the substitute URL is reachable without a session cookie.

### Origin allowlist

Empty `auth.allowed_origins` keeps the v0.3.0b behaviour: any well-formed `?origin=` value passes. A non-empty list restricts the token service to exact-match origins; anything else returns `AuthAccessTokenError2 { profile: "invalidOrigin" }`.

## Sidecar metadata

Each image may carry a TOML sidecar at `<images>/<id>.toml` (or `<images>/<subdir>/<id>.toml`). The Manifest builder picks it up and merges fields:

```toml
label = "The Creation of the World"
language = "en"             # default lang for label/summary; defaults to "none"
summary = "Genesis depicted in a medieval manuscript illumination."
rights = "https://creativecommons.org/licenses/by/4.0/"

[[metadata]]
label = "Date"
value  = "13th century"

[[metadata]]
label = "Source"
value  = "Bibliothèque nationale de France"

[provider]
id       = "http://example.org/bnf"
label    = "Bibliothèque nationale de France"
homepage = "https://www.bnf.fr/"
```

Without a sidecar the Manifest falls back to the filename stem as label and carries no metadata/provider/rights.

## Architecture

```
iiif-server/
├── crates/
│   ├── iiif-core          # Shared types, config, errors, storage trait
│   ├── iiif-image         # Image API 3.0 (processing pipeline)
│   ├── iiif-presentation  # Presentation API 3.0 (manifests, collections)
│   ├── iiif-auth          # Authorization Flow API 2.0
│   ├── iiif-search        # Content Search API 2.0
│   ├── iiif-state         # Content State API 1.0
│   ├── iiif-discovery     # Change Discovery API 1.0
│   └── iiif-server        # Binary, wires everything together
├── config.toml
├── Dockerfile
└── docker-compose.yml
```

Each IIIF specification is implemented as an independent crate with its own types, handlers, and tests. The `iiif-server` crate composes them into a single HTTP server.

## Development

```bash
cargo build                  # Compile
cargo test                   # Run all tests (180 unit + integration)
cargo clippy -- -D warnings  # Lint (zero warnings required)
cargo fmt --check            # Check formatting
cargo doc --open             # Generate documentation
```

## License

MIT License. See [LICENSE](LICENSE) for details.
