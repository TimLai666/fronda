## 1. FCPXML parser

- [x] 1.1 `parse_rational_seconds` converts `"N/Ds"`/`"Ns"`/`"0s"` to seconds and rejects a zero denominator / non-numeric input. Verified by `fcp_parse_rational_seconds_forms`.
- [x] 1.2 `parse_fcpxml` reads fps from the `<project>` sequence's referenced `<format>` frameDuration, scoped inside `<project>` so nested-media sequences in `<resources>` do not shadow it. Verified by `fcp_roundtrip_tracks_timing_and_files` (fps 30) and the nested-scope reasoning captured in the spec.
- [x] 1.3 Lane assignment inverts to track order (video high→low = tracks[0..] top-first, audio -1/-2/…), preserving clip offset/duration/trim. Verified by `fcp_roundtrip_tracks_timing_and_files`.
- [x] 1.4 Source-timecode origin is subtracted to recover the true trim in-point. Verified by `fcp_roundtrip_reads_source_timecode_origin`.
- [x] 1.5 Referenced files are collected (dedup by asset id) from `<media-rep src>` for host relink. Verified by the file-path assertions in `fcp_roundtrip_tracks_timing_and_files`.
- [x] 1.6 Retimed clips import at 1× with a note; Premiere/Resolve remain `NotImplemented`. Verified by `fcp_retimed_clip_imports_at_1x_with_note` and `import_xml_dispatches_parsers_and_stubs`.

## 2. Dispatch + gates

- [x] 2.1 `import_xml` routes `Fcpxml` to `parse_fcpxml` (was `NotImplemented`). Verified by `import_xml_dispatches_parsers_and_stubs`.
- [x] 2.2 Full `render_core` suite green (27 `xml_import` tests). Verified by `cargo test -p render_core xml_import` exit 0.
- [x] 2.3 Workspace + desktop-app gates green. Verified by `cargo test --workspace` and `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0.
