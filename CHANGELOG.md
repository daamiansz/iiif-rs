# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-15

### Added

- **Image API 3.0** — Full IIIF Image API implementation
  - Complete image processing pipeline: region, size, rotation, quality, format
  - Arbitrary rotation with bilinear interpolation
  - Formats: JPEG, PNG, WebP, GIF, TIFF
  - info.json with full Level 2 compliance
  - ETag, Last-Modified, 304 Not Modified caching
  - Canonical and Profile Link headers
  - Content negotiation for info.json
  - Passed official IIIF Image API validator (24/24 tests)

- **Presentation API 3.0** — Manifest and Collection serving
  - Auto-generated Manifests from images with Canvas + Annotation structure
  - Root Collection listing all available Manifests
  - JSON-LD with proper @context, linking properties, and thumbnails
  - Passed official IIIF Presentation validator (0 errors, 0 warnings)

- **Authorization Flow API 2.0** — Access control
  - Active login pattern with HTML login form
  - Cookie-based session management
  - Token service with postMessage iframe support
  - Probe service with Bearer token validation
  - Middleware protecting configured image identifiers
  - Logout with session invalidation

- **Content Search API 2.0** — Full-text annotation search
  - In-memory inverted index with tokenization
  - AND-logic multi-term search
  - Autocomplete with prefix matching and occurrence counts
  - Ignored parameter reporting
  - SearchService2 / AutoCompleteService2 descriptors

- **Content State API 1.0** — State encoding and sharing
  - Base64url encode/decode (RFC 4648)
  - Three content state forms: Annotation, URI, Target Body
  - Validation and sanitization
  - Encode, decode, and roundtrip endpoints

- **Change Discovery API 1.0** — Activity stream tracking
  - OrderedCollection with chronological pagination
  - Activity types: Create, Update, Delete
  - ISO 8601 timestamps
  - Page-based navigation with prev/next links

- **Infrastructure**
  - Cargo workspace with 8 crates
  - Fully asynchronous (tokio + spawn_blocking for CPU work)
  - Docker and docker-compose support
  - Configuration via TOML file and/or environment variables
  - Structured logging (tracing)
  - CORS, gzip/brotli compression
  - Graceful shutdown
