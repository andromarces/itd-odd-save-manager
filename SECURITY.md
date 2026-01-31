# Security Policy

## Supported Versions

The project supports the latest release version. The most recent version available on [GitHub Releases](https://github.com/andromarces/itd-odd-save-manager/releases) should be used.

| Version | Supported |
| ------- | --------- |
| Latest  | Yes       |
| Older   | No        |

## Reporting a Vulnerability

If a security vulnerability is discovered within this project, responsible disclosure should be prioritized.

1.  **Public GitHub issues should NOT be created for vulnerability reports.**
2.  Reports should be sent to andromarces@gmail.com (or the profile contact info).
3.  Reports should include a detailed description of the vulnerability and steps to reproduce it.

Receipt of reports will be acknowledged, and a fix or mitigation will be provided in a timely manner.

## Integrity Verification

All official releases are built via GitHub Actions. The integrity of the download can be verified by checking the `SHA256SUMS.txt` file included in every release and comparing it against the hash of the downloaded file.

## Network Activity

This application is designed to be local-first. It does not send analytics or telemetry.
*   **Updates**: Automatic updates are not currently implemented.
*   **Cloud Sync**: No cloud sync is currently implemented. All files remain on the local disk.
