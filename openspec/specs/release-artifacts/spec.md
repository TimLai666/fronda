# release-artifacts Specification

## Purpose

TBD - created by archiving change 'release-pipeline'. Update Purpose after archive.

## Requirements

### Requirement: Tagged and manual releases produce static-ffmpeg binaries per platform

A release workflow SHALL, on a `v*` tag push or manual `workflow_dispatch`, build the `--release` desktop binary (`fronda-app-shell-gpui`, `desktop-app` feature) for Linux, macOS, and Windows, with ffmpeg statically linked into each binary via vcpkg so the binary needs no ffmpeg installed at runtime. Each platform's binary SHALL be packaged into a per-platform archive and uploaded as a downloadable workflow artifact; on a tag push the archives SHALL additionally be attached to the corresponding GitHub Release. The release workflow SHALL be independent of the PR CI workflow.

#### Scenario: Manual dispatch builds all platforms

- **WHEN** the release workflow is triggered via workflow_dispatch
- **THEN** the Linux, macOS, and Windows jobs each build the desktop binary with static ffmpeg and upload a per-platform archive artifact

#### Scenario: Tag attaches archives to the release

- **WHEN** a `v*` tag is pushed
- **THEN** each platform's archive is attached to that tag's GitHub Release

#### Scenario: Released binary needs no system ffmpeg

- **WHEN** a released binary decodes a video thumbnail on a machine without any ffmpeg installed
- **THEN** decoding succeeds using the statically linked ffmpeg
