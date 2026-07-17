## ADDED Requirements

### Requirement: Speech spans invert to dead air

audio_core SHALL provide a pure function converting speech spans (seconds, source time) into removable non-speech ranges over a clip's duration, applying edge padding to each removable range's boundaries and dropping ranges shorter than the minimum silence duration.

#### Scenario: Gap between speech becomes a cut

- **WHEN** speech spans are [0.0, 2.0] and [5.0, 8.0] over a 8.0s clip with 0.1s padding and 0.5s minimum
- **THEN** the resulting removable range is (2.1, 4.9)

##### Example: Edges and short gaps

| speech spans | duration | padding | min | removable |
|---|---|---|---|---|
| [1.0,7.0] | 8.0 | 0.1 | 0.5 | (0.0,0.9), (7.1,8.0) |
| [0.0,3.0],[3.3,8.0] | 8.0 | 0.1 | 0.5 | (none — 0.3s gap under min) |

### Requirement: Analyzer-first detection with RMS fallback

The remove_silence detection path SHALL consult the injected SpeechAnalyzer first; when it returns spans they define the dead-air ranges, and when the analyzer is absent or returns None the detection SHALL fall back to the existing RMS adaptive-threshold path with identical behavior to today.

#### Scenario: No analyzer installed

- **WHEN** remove_silence runs on an executor with no SpeechAnalyzer
- **THEN** detection uses the RMS envelope path and results match the current implementation

#### Scenario: Analyzer provides spans

- **WHEN** a SpeechAnalyzer returns speech spans for a clip's source
- **THEN** the cut ranges derive from inverting those spans, not from the RMS envelope
