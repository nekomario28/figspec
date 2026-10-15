# 01 — model, checks, renderer, CLI

The model (`Figure`, `Mark`), the checks with stable codes, the mapping onto
kuva, and a CLI with the family's exit policy (0/1/2).

Checkpoint: `cargo test` passes; a bar+line model renders to SVG and PNG; the
same model renders to identical bytes; an unknown JSON field is refused; a bar
mark whose categories and values disagree is an error naming the mark.
