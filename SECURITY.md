# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | Yes                |

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly:

1. **Do NOT** open a public GitHub issue
2. Email the maintainer directly or use GitHub's private vulnerability reporting
3. Include a description of the vulnerability, steps to reproduce, and potential impact
4. Allow reasonable time for a fix before public disclosure

We aim to acknowledge reports within 48 hours and provide a fix within 7 days for critical issues.

## Security Considerations

This server implements the IIIF Authorization Flow API 2.0. For production deployments:

- Always use HTTPS (TLS) in production
- Change default credentials in `config.toml`
- Use strong, unique passwords for auth users
- Set appropriate `max_width`, `max_height`, and `max_area` limits to prevent resource exhaustion
- Review `protected` patterns to ensure sensitive images are covered
- Consider running behind a reverse proxy (nginx, Caddy) for TLS termination and rate limiting
