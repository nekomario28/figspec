# 03 — distribution

Wheels for Linux/macOS/Windows across the supported Pythons (abi3), a PyPI
release, and a consumer check that installs the wheel in a fresh environment and
renders a figure — the same shape the sibling project uses, so a published
artifact is proven rather than assumed.

Open question: the distribution name (`figspec` is free on PyPI and crates.io as
of 2026-10-15).

## Dependency plan

- **0.1 ships without matplotlib.** `pip install figspec` brings the model, the
  checks, the renderer and the Python surface, and no numpy. `figspec` cannot
  import matplotlib unless the caller asks for it (`figspec[matplotlib]`), and the
  renderer never imports it at all.
- **A later update moves matplotlib into the default install**, so
  `figspec.from_matplotlib` works out of the box for people who already have a
  matplotlib workflow. The condition for doing it: matplotlib must not become
  required by *importing* figspec. The dependency lives in the package metadata;
  `import figspec` stays numpy-free and `figspec.compile`/`render` stay
  matplotlib-free. If that cannot be kept, it stays an extra.
- **The reason to delay it** is measured, not stylistic: matplotlib is a 30 MB
  install with nine hard dependencies, and the main reason to reach for this
  project is a figure that can be checked and drawn without them.

## Building an uploadable wheel

A local `maturin build` produces `figspec-0.1.0-cp39-abi3-linux_x86_64.whl`, and
maturin warns that PyPI will not take it. Forcing the policy says why, exactly:

```
$ maturin build --manylinux 2_28 -o dist
Error ensuring manylinux_2_28 compliance
  Your library is not manylinux_2_28 compliant because of the presence of
  too-recent versioned symbols:
    libm.so.6 offending versions: GLIBC_2.29, GLIBC_2.43
    libc.so.6 offending versions: GLIBC_2.30, GLIBC_2.39, GLIBC_2.33, GLIBC_2.34
```

So the uploadable wheels are built in the manylinux container (`maturin-action`
with `manylinux: auto`, target `x86_64` and `aarch64`), which is a CI job rather
than a local command. The local wheel still matters: it is what
`scripts/check_consumer.py` installs in a fresh environment to prove the package
works when it is not a checkout. Windows and macOS wheels are added to the same
job when the first release is cut (abi3 means one wheel per platform, not one per
Python).
