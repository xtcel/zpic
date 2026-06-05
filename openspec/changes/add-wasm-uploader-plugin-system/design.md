## Context

`zpic` currently models uploaders as a closed set of built-in types. The config layer stores uploader settings in a flexible PicGo-shaped structure, but runtime resolution still collapses those settings into a built-in `UploaderKind` enum and a hard-coded factory. That design blocks runtime-extensible uploaders and incorrectly treats unknown uploader types as unsupported.

This change introduces a `zpic`-native plugin system with these constraints:

- PicGo compatibility remains config-only. We will read/import PicGo configs, but we will not run PicGo Node plugins or mimic PicGo's plugin command ecosystem.
- MVP support is uploader plugins only.
- The architecture should leave clear insertion points for future `transformer`, `hook`, MCP-facing, and AI-assisted flows without making them public requirements yet.
- The implementation needs to stay testable inside the Rust workspace and safe to call from future editor/agent integrations.

## Goals / Non-Goals

**Goals:**

- Allow `zpic upload` and `zpic migrate` to run either built-in uploaders or installed WASM uploader plugins.
- Keep uploader config storage in the existing PicGo-compatible shape: `uploader.<type>.configList`.
- Add manifest-driven plugin discovery, field schemas for guided setup, and doctor checks.
- Introduce a capability registry and pipeline structure that can later host `transformer` and `hook` stages without another large refactor.
- Keep existing JSON payloads and exit-code behavior stable for integrations.

**Non-Goals:**

- Running PicGo Node plugins or reproducing PicGo's plugin command behavior.
- Shipping plugin installation/publishing commands in this change.
- Exposing transformer or hook plugins to users in this change.
- Designing MCP as a plugin type. MCP stays a caller/surface concern.
- Shipping direct AI features in this change. AI is treated as a future host service boundary.

## Decisions

### 1. Introduce a dedicated plugin crate and a registry layer

Add a new workspace crate, tentatively `zpic-plugins`, that owns:

- plugin manifest parsing
- plugin discovery from standard directories
- uploader schema metadata
- runtime loading/execution
- a unified uploader registry API

The CLI and pipeline will resolve uploaders through this registry instead of talking directly to the built-in uploader factory.

Rationale: the current built-in factory is too narrow for runtime extensions, and we want a single resolution path for built-ins and plugins.

Alternatives considered:

- Keep resolution in `zpic-uploaders`: rejected because that crate is intentionally about first-party uploader implementations, not discovery and sandboxing.
- Put plugin logic in `zpic-cli`: rejected because future MCP or library consumers would have to duplicate the same runtime logic.

### 2. Keep built-in uploader implementations, but wrap them as registry providers

Built-in uploaders remain native Rust implementations, but the registry will expose them through the same descriptor model used for plugins: type name, display name, guided field schema, aliases, and factory/runner hook.

Rationale: this removes special cases from guided setup, doctor, and uploader resolution.

Alternatives considered:

- Leave built-ins on a separate path: rejected because it would duplicate behavior and make plugin support feel bolted on.

### 3. Use Wasmtime as the first runtime and define a small JSON-over-memory ABI for MVP

The first runtime implementation will use `wasmtime`. For the MVP ABI, plugins are core WASM modules that export memory plus a small set of functions used by the host to pass JSON requests and receive JSON responses. The registry/runtime layer will be shaped so a future WIT/Component Model ABI can replace or sit beside this transport.

Rationale: Wasmtime gives us a strong Rust embedding story and resource controls, while a small JSON ABI keeps the first implementation practical and testable inside this repo without needing a separate guest toolchain or component packaging workflow.

Alternatives considered:

- Wasmtime Component Model/WIT immediately: attractive long-term, but heavier guest authoring and test setup for the first cut.
- Wasmer or WasmEdge first: rejected for now because Wasmtime is the best fit for a Rust-first embedded host and smallest operational surface for this repo.
- Native process plugins: rejected because the team explicitly prefers WASM isolation.

### 4. Make manifest metadata the source of truth for plugin discovery and guided setup

Each plugin directory will contain:

- `plugin.toml` with plugin metadata
- `plugin.wasm` with the executable module

The manifest declares uploader types, optional PicGo config aliases, field schema, and future capability slots. Guided `set uploader` will consume this schema for plugin uploader types.

Rationale: plugin metadata needs to be available without instantiating the plugin runtime just to render prompts.

Alternatives considered:

- Derive schema from the WASM guest at runtime: rejected because discovery and UX become slower and less predictable.

### 5. Preserve the PicGo-shaped config model, but stop coercing unknown uploader types into built-in kinds

The config model will continue to store uploader config groups by raw type string. Runtime resolution will treat the uploader type as an opaque key first, then ask the registry whether that key maps to a built-in or plugin uploader. Built-in-only conversion helpers will become explicit instead of defaulting unknown types to `local`.

Rationale: the current fallback is unsafe for plugin types and hides resolution errors.

Alternatives considered:

- Replace the config model with a fully new native plugin structure: rejected because it breaks the migration ergonomics we want to keep.

### 6. Reserve pipeline extension points now, but only enable the uploader phase

Internally, the pipeline will be structured around stages such as:

- input preparation
- pre-upload transforms
- upload
- post-upload transforms
- formatting
- hooks

Only the upload stage is plugin-enabled in this change. The registry and manifest model may include placeholder capability buckets for future phases, but the host will ignore unsupported ones for now.

Rationale: this avoids another large refactor when transformer/hook work starts, without overcommitting public behavior.

Alternatives considered:

- Delay all staging work until after uploader plugins: rejected because the uploader refactor is the cleanest moment to define the boundary once.

### 7. Treat MCP and AI as host-side service boundaries, not plugin categories

Future MCP support should call the same app/service layer that the CLI uses. Future AI-assisted features should enter through host services invoked by the pipeline or plugins under explicit permissions.

Rationale: MCP is a surface, not a backend capability, and AI calls have security/cost implications that should stay host-managed.

Alternatives considered:

- Define `mcp` or `ai` as plugin capability types now: rejected because it mixes invocation surfaces with execution capabilities and would create premature API surface area.

## Risks / Trade-offs

- [WASM ABI may evolve] -> Hide runtime calls behind a `zpic-plugins` abstraction so we can add a Component Model backend later without rewriting CLI consumers.
- [Plugin discovery introduces config/setup ambiguity] -> Make `doctor` report discovery paths, manifest parse errors, and active-uploader resolution details explicitly.
- [Guided setup becomes more dynamic] -> Keep `--field key=value` as the lowest-common-denominator path even when schema metadata is missing.
- [PicGo plugin expectations may be confusing] -> Update docs and errors to state clearly that only PicGo config compatibility is supported.
- [Runtime dependency increases build/test cost] -> Keep plugin code in a dedicated crate and add focused unit/integration tests rather than broad end-to-end duplication.

## Migration Plan

1. Add the plugin crate, manifest types, discovery logic, and uploader registry abstraction.
2. Refactor uploader resolution to use raw uploader type strings and registry lookups.
3. Integrate Wasmtime-backed uploader execution and plugin-aware doctor/setup flows.
4. Update docs/tests/specs to reflect the new plugin system and the PicGo config-only compatibility boundary.

Rollback strategy:

- If runtime plugin execution proves unstable before release, built-in uploaders can continue to operate through the registry abstraction while plugin discovery/loading is disabled behind targeted code changes.

## Open Questions

- Should we expose a `zpic plugin list` command in the next change, or keep discovery filesystem-driven for now?
- Do we want a project-local plugin directory in addition to a user-global plugin directory in the first cut?
- When we move to Component Model/WIT, do we keep the JSON ABI as a compatibility mode or treat it as an internal transitional layer only?
