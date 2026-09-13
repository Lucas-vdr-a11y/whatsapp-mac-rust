# Security Policy

RustWA handles WhatsApp account credentials and private message content. We
take that seriously.

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Use GitHub's
private vulnerability reporting on this repository (Security → Report a
vulnerability). You should get a response within a few days.

## Scope

In scope:

- Session/key material leaking outside the local application-support directory
- Memory-safety issues in the Rust code
- Command injection or sandbox escapes through the Tauri IPC layer
- Cryptographic misuse in our integration with the upstream protocol library

Out of scope:

- Vulnerabilities in WhatsApp's servers or in the official apps
- Account bans resulting from using an unofficial client (see the README
  disclaimer — you accepted that risk by using this software)

## Known design choices

- Session credentials are stored on disk with restrictive permissions; OS
  keychain integration is tracked in the roadmap.
- The webview runs with a restrictive CSP and a minimal Tauri capability set;
  it never receives raw key material.
