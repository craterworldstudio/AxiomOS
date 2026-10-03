# Capability Lifecycle — State Mutation Exercise

## Copy

Initial:

Slot 1 ──► Endpoint E

Operation:

Copy(Slot 1, Slot 2)

Result:

Slot 1 ──┐
         ├──► Endpoint E
Slot 2 ──┘

- Slot 1 generation is unchanged.
- Slot 2 receives a new generation.
- Slot 2 receives a derived capability.
- Endpoint E is not duplicated.
- Slot 2 may receive equal or reduced rights.
- Slot 2 becomes a descendant of Slot 1 for revocation purposes.

## Delete

Initial:

Slot 1 ──┐
         ├──► Endpoint E
Slot 2 ──┘

Operation:

Delete(Slot 1)

Result:

Slot 1 ──X
Slot 2 ─────► Endpoint E

- Slot 1 becomes empty.
- Slot 1 generation advances.
- Slot 2 remains valid.
- Endpoint E remains alive.

## Revoke

Initial:

Slot 1 ──┐
         ├──► Endpoint E
Slot 2 ──┘
           \
            └── derived capabilities...

Operation:

Revoke(Slot 1)

Result:

Slot 1 ──X
Slot 2 ──X
derived capabilities ──X

- The source capability is revoked.
- All capabilities derived from that source are revoked.
- Independent capabilities to Endpoint E survive.
- Endpoint E itself survives if still referenced.

## Destroy

Initial:

Slot 1 ──┐
Slot 2 ──┼──► Endpoint E
Slot N ──┘

Operation:

Destroy(Endpoint E)

Result:

Slot 1 ──X
Slot 2 ──X
Slot N ──X

- Endpoint E enters the destroyed state.
- No capability may invoke it afterward.
- Destruction does not depend on capability lineage.
- Physical object storage is reclaimed only after all kernel references disappear.

## Core Distinction

Delete removes one authority edge.

Revoke removes an authority edge and its descendants.

Destroy invalidates the authority target itself.
