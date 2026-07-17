## ADDED Requirements

### Requirement: Cancellable FIFO export queue with staged output

Exports SHALL run through a FIFO queue with job states waiting/preparing/exporting/canceling/completed/failed/canceled, destination reservation rejecting duplicate queued paths, cancellation of both waiting and in-flight jobs, and staged output writes so a canceled or failed export leaves no partial file at the destination (upstream #298).

#### Scenario: Cancel an in-flight export

- **WHEN** an exporting job is canceled
- **THEN** the job transitions through canceling to canceled and the destination path has no partial file

#### Scenario: Duplicate destination rejected

- **WHEN** a second job is enqueued for an already-reserved destination
- **THEN** the enqueue fails until the first job releases the destination
