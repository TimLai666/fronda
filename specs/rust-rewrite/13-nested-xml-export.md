# 13 — Native nested-sequence XML/FCPXML export (upstream #255, final piece)

The #255 port is complete except for this: both exporters currently FLATTEN
nest carriers via `timeline_core::flatten_nests` (content-correct — the pixels
and audio match — but the nested structure is lost, so an NLE user can't open
the nest as a compound/nested sequence). Swift emits real nested structures.
This spec captures Swift's exact output (verbatim-audited from upstream
`e605633`) so the port lands in one pass.

> **STATUS 2026-07-05: XMEML half IMPLEMENTED** (write_sequence recursion +
> write_nest_clipitem inline-once/then-reference, frozen-carrier drop; the
> cur_seq_width Scale% fix proved N/A — Rust's Scale uses the normalized
> transform). XMEML carrier filters + root
> sequence name landed too. Remaining: the FCPXML half below only.

Current Rust seams: `FcpxmlExport::export_with_target_and_timelines(timeline,
manifest, target, timelines)` and
`XmlExport::export_with_manifest_and_timelines(timeline, manifest, timelines)`
— both have the sibling map and flatten at entry. Replace the flatten with the
structures below. All existing tests keep passing except
`fcpxml_export_flattens_compound_clip_to_nested_asset` (update it to assert the
native structure instead).

## FCPXML (fcpxml_export.rs, ~300-line main fn needs a story-nodes refactor)

**Prerequisite refactor:** extract the per-track/per-clip story-node emission
from the main fn into a reusable fn parameterized by timeline (Swift did the
same: `emittableClips(of: Timeline)` + `storyNodes(for:)` now take a timeline).
The lane assignment, `redundant_audio_clip_ids`, and per-clip adjustments all
move with it.

1. **Recursive media-ref collection** (BFS, replaces the flat scan):
   walk `[root]`; per timeline, `visited.insert(t.id)`; for each clip:
   `sourceClipType == sequence` → enqueue `resolve(mediaRef)`; else collect
   `mediaRef` into the asset set. Assets from EVERY reachable timeline get
   `<asset>` resources.

2. **collectNests**: reachable (same BFS, maxDepth 8 = `NEST_MAX_DEPTH`),
   NON-EMPTY (`totalFrames > 0`) nested timelines in discovery order; assign
   media ids `nest1`, `nest2`, …; `nestIndex: child.id → mediaId`.

3. **nestFormatNode** (emit only when the child canvas differs from the parent
   format): `<format id={nestFormatId} name={sequence_format_name(child.w,
   child.h, fps)} frameDuration={frame_duration_str(fps)} width height
   colorSpace="1-1-1 (Rec. 709)"/>`. NOTE: frameDuration uses the PARENT
   project fps (Swift passes `Double(fps)` of the exporter).

4. **nestMediaNode** per nest — the compound resource:
   ```xml
   <media id="nest1" name="{child.name}">
     <sequence format="{nestFormatId}" duration="{time(child.totalFrames)}"
               tcStart="0s" tcFormat="NDF" audioLayout="stereo" audioRate="48k">
       <spine>
         <gap name="Timeline" offset="0s" start="0s" duration="{same}">
           <!-- storyNodes for the child's emittable clips (recursive: a child
                may itself contain ref-clips over deeper nests) -->
         </gap>
       </spine>
     </sequence>
   </media>
   ```

5. **assetClipNode sequence branch** — a carrier emits `<ref-clip>`:
   - `ref={nestIndex[mediaRef]}` (bail `None` if unresolved), `name={child.name}`,
     `lane`, `offset={time(startFrame)}`, `start={time(trimStartFrame)}`,
     `duration={time(min(durationFrames, max(0, child.totalFrames -
     trimStartFrame)))}` (clamp a frozen carrier to current child length; bail
     if ≤ 0), `enabled`.
   - AUDIO carrier: attrs += `srcEnable="audio"`; children = volume node only.
   - VIDEO carrier: if it has NO linked audio partner, attrs +=
     `srcEnable="video"`; children = `<adjust-conform type="fit"/>`, crop,
     transform, blend, and (when a linked audio partner was collapsed) that
     partner's volume node — same helpers as asset-clips (`clip_adjustments`
     pieces).
   - Linked-pair grouping: `.sequence` goes in the VIDEO bucket (with `.video`
     and `.image`).

6. **isEmittable**: sequence carrier → `nestIndex.contains(mediaRef)`;
   `durationFrames > 0` still required.

7. **Project name**: Swift also switched `<project name>` from "Timeline
   Export" to `timeline.name` — port that in the same change.

## XMEML (xml_export.rs)

1. Builder state: `sequence_ids: HashMap<child_id, String>` ("sequence-N"),
   `emitted_sequences: HashSet<child_id>`, and `cur_seq_width` — the CURRENT
   sequence's canvas width, swapped while emitting a nested sequence (Scale% =
   `cur_seq_width / sourceWidth * transform.width * 100`, NOT the root width —
   Swift fixed this in #255).

2. **sequenceNode(id, timeline)** — refactor the root `<sequence>` emission
   into a fn used for the root AND recursively per nest: name =
   `timeline.name`, duration = totalFrames, rate = PARENT fps, own
   width/height in the video format node. Save/restore per-sequence state
   (clip addresses, link groups, cur_seq_width) around the recursion.

3. **nestClipItemNode(clip, is_audio)** — a carrier's `<clipitem>`:
   masterclipid, `name = child.name`, `duration = child.totalFrames`, rate,
   `start/end = startFrame / startFrame + (out - in)`, `in = trimStartFrame`,
   `out = min(in + durationFrames, child.totalFrames)`; then the full
   `<sequence id>` node on FIRST use, a self-closing `<sequence id="..."/>`
   reference on later uses (Premiere convention); then
   volumeFilters/videoFilters + link nodes as usual.

4. **sortEmittable** drops a sequence carrier whose `trimStartFrame >=
   child.totalFrames` (would emit out < in).

## Testing plan

- FCPXML: nest → assert `<media id="nest1"` + `<ref-clip ref="nest1"` present,
  child's asset emitted once, carrier trim/duration clamped; two carriers over
  the SAME child → one `<media>`, two `<ref-clip>`s; audio carrier gets
  `srcEnable="audio"`; two-level nest emits both media nodes.
- XMEML: first use emits the full `<sequence id="sequence-1">` inline, second
  use a reference; Scale% inside the nested sequence uses the CHILD canvas.
- Keep `flatten_nests` for `mix_timeline_audio_with_timelines` (audio mixing
  still flattens — only the exporters go native).

## Size estimate

FCPXML ~250 LoC (mostly the story-node refactor), XMEML ~200 LoC. Both are
pure string builders with existing golden tests to protect the non-nested
output byte-for-byte.
