# Security policy

## Supported versions

Only the latest release on the default branch is currently supported with security fixes. Development snapshots may contain unfinished behavior.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's private security advisory reporting for this repository, or contact the maintainer privately through the address configured in the GitHub security settings.

Include the affected version, reproduction steps, impact, and logs with secrets removed. Never include course files, passwords, session cookies, API tokens, or private network details.

Keep `.env` and persistent data outside the repository. Run the container with the library mounted read-only and use HTTPS through the local reverse proxy.
