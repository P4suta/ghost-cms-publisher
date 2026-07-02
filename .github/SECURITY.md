# Security Policy

## Supported versions

ghost-cms-publisher is pre-1.0. Only the latest released version receives
security fixes.

| Version | Supported |
| ------- | --------- |
| latest  | ✅        |
| older   | ❌        |

## Reporting a vulnerability

Please report vulnerabilities **privately** via GitHub Security Advisories:
open the repository's **Security → Report a vulnerability** page. Do not file a
public issue for a security report.

We aim to acknowledge a report within a few days and to ship a fix or mitigation
before any public disclosure.

## Scope note — treat the Staff Access Token as a credential

The Ghost **Staff Access Token** this tool consumes is equivalent to the account
it belongs to. Any defect that could cause the token (or a JWT minted from it) to
leak is in scope — for example if it could be written to `ghost-cms.toml`, logs,
error output, insta snapshots, or the publish-state cache. Reports of such
exposure paths are especially welcome.
