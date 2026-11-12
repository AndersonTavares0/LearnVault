# Security policy

## Supported versions

Security fixes apply to the latest release from the default branch. Development snapshots may contain unfinished behavior.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's private security advisory reporting for this repository, or contact the maintainer through a private channel configured for the project.

Include the affected version, reproduction steps, impact, and logs with secrets removed. Never include course files, passwords, session cookies, API tokens, or private network details.

Keep `.env` files and persistent data outside version control. Mount the course library read-only and use HTTPS through a reverse proxy for network deployments.

## What to expect

- **First response:** the maintainer aims to acknowledge a private report within seven days.
- **Fix and disclosure:** once the issue is confirmed and a fix is released, the maintainer publishes a GitHub security advisory that describes the issue, its impact, the affected versions, and the fixed version.
- **Credit:** reporters are credited in the advisory unless they ask not to be.
