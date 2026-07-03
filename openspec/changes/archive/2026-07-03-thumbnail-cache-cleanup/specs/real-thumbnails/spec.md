## ADDED Requirements

### Requirement: Thumbnail cache evicts stale and excess entries

The thumbnail cache SHALL bound its growth. After a new thumbnail is written for a source, prior cached files for that same source (same hash prefix, different mtime) SHALL be removed. On app startup a background pass SHALL prune the cache directory to a fixed size cap (256 MB), deleting oldest files first until under the cap; when already under the cap it deletes nothing. All cleanup failures SHALL be silent and MUST NOT affect the app.

#### Scenario: Source update removes the old thumbnail

- **WHEN** a source's mtime changes and a fresh thumbnail is written
- **THEN** the previous thumbnail for that source is removed and unrelated sources' thumbnails remain

#### Scenario: Size cap prunes oldest first

- **WHEN** the cache exceeds the size cap at startup
- **THEN** the oldest files are deleted until the total is under the cap

#### Scenario: Under-cap cache is untouched

- **WHEN** the cache is under the size cap at startup
- **THEN** no files are deleted

##### Example: Prune order

| File | mtime | size |
| ---- | ----- | ---- |
| a.png | oldest | deleted first |
| b.png | middle | deleted next if still over cap |
| c.png | newest | kept |
