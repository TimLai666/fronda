This folder contains the model-build tooling used by Fronda's visual search stack and the legacy Palmier compatibility path.

Relevant specs:

- `specs/rust-rewrite/06-search-transcription-generation-and-shell.md`
- `specs/rust-rewrite/10-current-status-and-plan.md`
- `specs/rust-rewrite/11-identifier-migration-plan.md`

## Semantic search

We use a CLIP-family model to empower visual search in the editor, where agents and users can search
through footage with CLIP model running locally. The model is downloaded during runtime and
not bundled in the app.

## The model

We use SigLIP 2 (https://huggingface.co/google/siglip2-base-patch16-256) by Google. The current artifact pipeline here still targets Apple's Core ML format for compatibility with the inherited search implementation and related fixtures.

## Building the Core ML packages

There's no official Core ML build, so we convert it ourselves:

```
uv venv --python 3.12 .venv
uv pip install -p .venv/bin/python -r requirements.txt
.venv/bin/python convert.py --checkpoint checkpoint --out build-q8 --palettize-bits 8
```

(See convert.py for how to fetch the checkpoint first.) The script traces both
encoders to .mlpackage, quantizes to 8-bit, and aborts unless the converted
model's embeddings match PyTorch's (cosine >= 0.99). `export_tokenizer.py`
regenerates the legacy tokenizer golden tests.

## Hosting

The build output (two encoder zips, tokenizer.zip, manifest.json) is uploaded to
huggingface.co/palmier-io/siglip2-base-coreml.

## Download

The legacy Swift baseline downloads the artifacts from the repo above on first use and verifies
them against the sha256s pinned in `Sources/PalmierPro/Search/SearchIndexConfig.swift`, which is also where
the URL currently lives.

## License

SigLIP 2 weights are Apache 2.0 (Google); our converted artifacts are
redistributed under the same terms, with attribution in the HF model card.
