# operai-abi

Stable ABI types for Operai Tool runtime.

This crate defines the FFI boundary between the Operai Toolbox runtime and dynamically loaded tool libraries (cdylib). All types use `abi_stable` for guaranteed ABI stability across Rust compiler versions.

## Ownership Philosophy

Types in this crate follow the principle: **borrow as much as possible until cloning is needed** (e.g., for async futures).

- **`'static` lifetime types** (`ToolDescriptor`, `ToolMeta`): Borrow from the loaded library. The data lives as long as the library is loaded.

- **Per-call types with `'a` lifetime** (`CallContext`, `CallArgs`): Borrow from the caller's stack. Valid only for the duration of the synchronous FFI call.

- **Async return types** (`FfiFuture`): FFI-safe futures returned from tool operations. The caller awaits these to get the result.

- **SDK types** (`Context` in operai): Owned for user ergonomics. The SDK clones data from the FFI types when crossing the async boundary.

## Safety

This crate uses `abi_stable` to provide safe FFI types. Tool authors should use the `operai` SDK which provides additional abstractions.
