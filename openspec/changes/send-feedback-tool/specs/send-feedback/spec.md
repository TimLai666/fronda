## ADDED Requirements

### Requirement: send_feedback agent tool

The agent tool surface SHALL include a `send_feedback` tool taking a `message` string, which submits product feedback through the injected FeedbackSender host seam. When no sender is connected the tool SHALL return an "unavailable" error naming the missing capability, mirroring the remove_silence no-decoder boundary.

#### Scenario: Feedback sent through the seam

- **WHEN** the agent calls send_feedback with a non-empty message and a FeedbackSender is installed
- **THEN** the sender receives a payload containing the message and diagnostics, and the tool reports success

#### Scenario: No sender installed

- **WHEN** send_feedback is called on an executor without a FeedbackSender
- **THEN** the tool returns an error stating feedback is unavailable and no state changes

### Requirement: Session dedup and cap

The executor SHALL reject a message identical to one already sent in the current session, and SHALL reject any send after 8 successful sends in the session (upstream #152 semantics), each with a distinct explanatory error.

#### Scenario: Duplicate message rejected

- **WHEN** send_feedback is called twice with the same message in one session
- **THEN** the second call returns a duplicate-feedback error and the sender is not invoked again

#### Scenario: Session cap reached

- **WHEN** 8 feedbacks were already sent this session and a 9th distinct message is submitted
- **THEN** the tool returns a session-limit error and the sender is not invoked
