# craft-bridge

Rebuild [SolidCraft](https://github.com/tangency-labs/solidcraft) designs inside
SolidWorks as **native parametric models** — replay, not translation.

Trying a new CAD system on real work should not be a one-way door. craft-bridge
takes a design built in SolidCraft and rebuilds it in your own SolidWorks with a
real feature tree: sketches with their relations and driving dimensions, features,
and parameters as equations — so you (or a client who expects SolidWorks files)
can keep editing in SolidWorks.

## How it works

1. **Export** a SolidCraft design as a recipe plus geometry, and **pack** it into
   a `.craftx` artifact with the bridge CLI.
2. **Generate** a SolidWorks macro (plain-text `.swb`) from the artifact.
3. **Run** the macro in SolidWorks 2018 or later (`Tools > Macro > Run`).
4. **Get** a native `.sldprt` with a feature tree, plus a report of exactly what
   was rebuilt, what was degraded, and what became a fallback body.

Steps that cannot be replayed fall back to an embedded body instead of failing the
model, and nothing happens silently: you see a plan report before running and a
runtime report after.

You can review the generated macro as plain text before it runs — it is your file.

## Usage

The bridge CLI packs and inspects `.craftx` artifacts today (ticket #2 of the
v1 plan); macro generation lands next. Built with Rust (`cargo build --release`).

```bash
# Pack a design exported from SolidCraft (recipe JSON + STEP geometry).
# --warnings takes the exporter's omission warnings as a JSON array of strings;
# without it the manifest records them as not provided.
craft-bridge pack --recipe script.json --step part.step \
  --warnings warnings.json --out part.craftx

# Show what an artifact contains (add --json for machine-readable output).
craft-bridge inspect part.craftx
```

The artifact contract is [docs/replay-profile.md](docs/replay-profile.md).

## Status

Program spec published (see [docs/spec.md](docs/spec.md)); v1 (single parts) in
development. Assemblies, configurations and a SolidWorks add-in are later phases;
their decisions are recorded in the spec.

## Documents

- [Program spec](docs/spec.md)
- [Glossary](GLOSSARY.md)
- ADRs: [replay, not translation](docs/adr/0001-replay-not-translation.md) ·
  [exemption and hard boundary](docs/adr/0002-exemption-and-boundary.md) ·
  [`.swb` text as the artifact](docs/adr/0003-swb-text-artifact.md)

## Disclaimers

- SOLIDWORKS is a registered trademark of Dassault Systèmes. This project is not
  affiliated with, endorsed by, or sponsored by Dassault Systèmes; references are
  nominative, to describe interoperability.
- The generated macros are plain text you can read before running. They run only
  where a licensed SolidWorks is installed, under that license's terms.
- Distributing tools that connect to the SolidWorks API may require Dassault's
  Solution Partner program. This project generates script text and does not ship
  API-connecting binaries in v1; the question is open, not resolved (see ADR 0003).
- No third-party materials (interop assemblies, documentation excerpts, icons)
  are committed to this repository.

## License

[Apache-2.0](LICENSE)
