## ADDED Requirements

### Requirement: Audio sync correlation enforces a minimum overlap

find_sync_offset SHALL exclude correlation lags whose envelope overlap is shorter than max(16 hops, 3 seconds) from peak selection (upstream #269 guard), returning None when no lag reaches the floor, so thin-edge overlaps can never win as spurious sync matches.

#### Scenario: Thin-edge overlap cannot win

- **WHEN** two signals only correlate strongly at a lag whose overlap is a few RMS frames
- **THEN** that lag is excluded and the reported peak lag satisfies the minimum-overlap bound

#### Scenario: Signals too short to overlap three seconds

- **WHEN** both signals are two seconds long
- **THEN** find_sync_offset returns None instead of a low-overlap match
