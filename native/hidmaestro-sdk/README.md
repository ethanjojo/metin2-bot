# HIDMaestro source adapter

This project compiles the pinned MIT HIDMaestro 2.1.0 sources with the original released resources. `tools/prepare-input-sdk.ps1` verifies the source archive and original deployment source, extracts all resources without invoking SDK methods, and runs the unmodified upstream catalog generator. It writes all generated files under `build/input-sdk/2.1.0`.

The single SDK policy change replaces `DriverDeploy.EnsureForCreate` with a fresh package/version check under the original machine deploy lock. Missing or older packages produce an installer-required error. Device creation cannot install, upgrade, add packages or update deployment records. Explicit management still uses the original SDK deployment policy.

`BridgeAccess.cs` permits the sibling `MetinInput` assembly to call the SDK's canonical read-only readiness check. The offline test executable reuses an existing upstream test friendship, substitutes a fake deployment machine, and checks refusal/readiness, lock release, zero mutations and byte-identical embedded resources. It never creates a context or native input device.

The original and replacement method snapshots make source drift a build error. Review both snapshots and the immutable source hash before any SDK update. Resource logical names containing backslashes are retained exactly through generated MSBuild resource entries.
