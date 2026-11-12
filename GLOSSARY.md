# Craft Bridge

Moving a design built in SolidCraft into SolidWorks as a living parametric model.

## Language

**Bridge**:
The whole craft-bridge system: packing a design into an artifact and rebuilding it in SolidWorks.
_Avoid_: connector, translator, converter

**Artifact**:
The packaged handoff between systems: a design's recipe, its fallback geometry and its metadata.
_Avoid_: export file, exchange file

**craftx**:
The container format an artifact is packed into.
_Avoid_: zip, package

**Recipe**:
The recorded, ordered list of model-building steps a design is made of, with its parameters.
_Avoid_: history, script, command log

**Replay**:
Rebuilding a recipe step by step using another system's own modeling commands.
_Avoid_: translation, conversion

**Replay profile**:
The versioned agreement on what an artifact contains and how a rebuild consumes it.
_Avoid_: contract, schema (when meaning the agreement as a whole)

**Native feature tree**:
The target outcome: a SolidWorks model whose features, sketches and equations are ordinary, editable SolidWorks objects.
_Avoid_: live model, editable history

**Fallback body**:
A stored shape inserted into the rebuild for a step that cannot be replayed, so the model still opens.
_Avoid_: dumb body, dead geometry

**Degraded sketch**:
A sketch rebuilt with its geometry and driving dimensions but without its full set of relations.
_Avoid_: simplified sketch, partial sketch

**Plan report**:
What the generator predicts about a rebuild before SolidWorks runs.
_Avoid_: validation report, dry-run log

**Runtime report**:
What actually happened inside SolidWorks when the macro ran.
_Avoid_: build log, result file

**Synthetic corpus**:
The purpose-built set of test designs and their artifacts that proves rebuild fidelity. Real client designs never enter it.
_Avoid_: test data, fixtures

**Seat**:
A licensed SolidWorks installation where acceptance runs happen.
_Avoid_: workstation, instance

**Hard boundary**:
The rule that nothing derived from SolidWorks crosses back into SolidCraft.
_Avoid_: firewall, one-way street
