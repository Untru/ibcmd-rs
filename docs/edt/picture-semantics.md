# Per-use picture semantics

`ibcmd-picture-semantics/1` is an explicit adapter source extension. It preserves
the independent `LoadTransparent` flag and transparent pixel of a reference
picture while retaining the same `Ref`. Installed EDT's `PictureRef` model has
only the reference; its exporter derives the flag from picture metadata and
does not emit a per-use pixel. The extension does not change picture identity,
create replacement CommonPictures, or claim the EDT model stores these values.

The versioned resource `ibcmd-picture-semantics.v1.json` lives beside a managed
form's `Form.form`. Its schema is
`urn:ibcmd:source-extension:picture-semantics:1`. Every row binds to the declared
form UUID, a command identity or control ancestry, the exact typed control kind
and id/name, a known picture slot, and the unchanged current reference.
Unknown fields, versions, slots, missing identities, duplicate bindings, changed
references and invalid pixels reject before output publication.

Choice-list pictures additionally bind to the ordered localized presentation
and the complete canonical typed choice value. The resource never parses or
replays embedded value text. Distinct choices can be reordered safely. Identical
projected choices use an explicit occurrence ordinal within their typed list;
reordering such indistinguishable occurrences requires updating that context.

Reference, flag, pixel and binding are serialized in the semantic fingerprint.
Both native and EDT readers derive all reference records from current typed
values. Resource selection retains only validated bindings and file presence;
rendering always uses current values. Metadata pixel edits cannot overwrite an
independent per-use flag. Descriptor projection keeps ordinary EDT-compatible
references; the accompanying resource carries values absent from EDT's model.

The conversion report explicitly lists the capability and the number of
resources and semantic carrier rows emitted or consumed. Ordinary reference
records that need no resource do not inflate this ledger. A project without
resources reports no extension use.

Adapter conversion of the resource is independent of `.ibcmd-provenance`.
Changing a pixel changes canonical semantics and native XML output; updating a
provenance file hash cannot authorize returning an old original. Unknown or
orphan resources remain subject to complete source inventory accounting.

Installed EDT import/validation of generated resources is a separate acceptance
gate. An EDT-only export may omit adapter resources and their additional
semantics. Preserve the resources when returning through the adapter; an SDK
export alone is not a lossless transport for these per-use values.
