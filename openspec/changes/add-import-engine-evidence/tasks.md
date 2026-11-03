# Tasks

- [x] 1. Specify borrowed diagnostic API and unsupported dimensions (Refs #382).
- [x] 2. Add isolated engine observer and synthetic regression test sources (Refs #382); executed within the integrated root-library gate; exact revision attribution is recorded in design.md.
- [x] 3. Obtain independent fresh source review and resolve findings (Refs #382); source-only APPROVE, strict stream and normalization provenance findings addressed.
- [x] 4. Run integrated Rust checks after coordinated build authorization (Refs #382); historical integration 8ce57d11 passed the full local 27-step gate/root library 4260 tests.
- [x] 5. Review the integrated passive engine_observation adapter and Capture::packed consumer, including paired bounds and typed-error tests (Refs #382). Future pre-reduction/root/versions authority hooks remain outside this delivery and require their own review.
