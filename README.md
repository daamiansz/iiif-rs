# IIIF Server

A complete [IIIF](https://iiif.io/) (International Image Interoperability Framework) server written in Rust, implementing all 6 IIIF specifications.

[![CI](https://github.com/daamiansz/iiif-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/daamiansz/iiif-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org/)

## Features

| Specification | Version | Status |
|---|---|---|
| [Image API](https://iiif.io/api/image/3.0/) | 3.0 | Level 2 compliance, validated |
| [Presentation API](https://iiif.io/api/presentation/3.0/) | 3.0 | Validated |
| [Authorization Flow API](https://iiif.io/api/auth/2.0/) | 2.0 | Active pattern |
| [Content Search API](https://iiif.io/api/search/2.0/) | 2.0 | Full-text + autocomplete |
| [Content State API](https://iiif.io/api/content-state/1.0/) | 1.0 | Encode/decode/validate |
| [Change Discovery API](https://iiif.io/api/discovery/1.0/) | 1.0 | Activity streams |

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
```

### Authorization Flow API 2.0

```
GET  /auth/login                    — Login page
POST /auth/login                    — Submit credentials
GET  /auth/token                    — Token service (iframe/postMessage)
GET  /auth/probe/{resource_id}      — Probe service (Bearer token)
GET  /auth/logout                   — Clear session
```

### Content Search API 2.0

```
GET /search?q={terms}&motivation={m}       — Full-text search
GET /autocomplete?q={prefix}               — Term suggestions
```

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
pattern = "active"
cookie_name = "iiif_access"
token_ttl = 3600
protected_dirs = ["restricted"]

[[auth.users]]
username = "admin"
password = "changeme"
```

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
cargo test                   # Run all tests (91 unit tests)
cargo clippy -- -D warnings  # Lint (zero warnings required)
cargo fmt --check            # Check formatting
cargo doc --open             # Generate documentation
```

## License

MIT License. See [LICENSE](LICENSE) for details.
