//! Tool registry for managing loaded tools.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use abi_stable::std_types::{RSlice, RStr};
use async_ffi::FfiFuture;
use operai_abi::{
    CallArgs, CallContext, CallResult, InitArgs, RuntimeContext, TOOL_ABI_VERSION, ToolModuleRef,
    ToolResult,
};
use rkyv::rancor::BoxedError;
use tracing::{info, instrument};

use crate::loader::{LoadError, ToolLibrary};

/// Errors that can occur when working with the registry.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    #[error("tool not found: {0}")]
    NotFound(String),

    #[error("failed to load library: {0}")]
    LoadError(#[from] LoadError),

    #[error("duplicate tool ID: {0}")]
    DuplicateId(String),

    #[error("tool invocation failed: {0}")]
    InvocationError(String),
}

/// Metadata about a registered tool.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// e.g., `"hello-world.greet"`
    pub qualified_id: String,
    /// Tool ID within the crate, e.g., `"greet"`
    pub tool_id: String,
    /// e.g., `"hello-world"`
    pub crate_name: String,
    pub crate_version: String,
    pub display_name: String,
    pub description: String,
    pub input_schema: String,
    pub output_schema: String,
    pub credential_schema: Option<String>,
    pub capabilities: Vec<String>,
    pub tags: Vec<String>,
    /// Pre-computed embedding for semantic search.
    pub embedding: Option<Vec<f32>>,
}

/// Handle to a tool for invocation.
pub struct ToolHandle {
    info: ToolInfo,
    module: ToolModuleRef,
    /// Serialized system credentials for this tool.
    pub system_credentials: Vec<u8>,
    /// Cached here to avoid repeated allocation when calling.
    tool_id: String,
}

impl ToolHandle {
    #[must_use]
    pub fn info(&self) -> &ToolInfo {
        &self.info
    }

    /// Invokes the tool, returning an FFI-safe future.
    #[instrument(skip(self, context, input), fields(tool_id = %self.info.qualified_id))]
    pub fn call(&self, context: CallContext<'_>, input: RSlice<'_, u8>) -> FfiFuture<CallResult> {
        let args = CallArgs::new(context, RStr::from_str(&self.tool_id), input);
        self.module.call()(args)
    }
}

/// Registry of loaded tools.
pub struct ToolRegistry {
    /// Kept alive to prevent dynamic libraries from unloading.
    libraries: Vec<ToolLibrary>,
    tools: HashMap<String, Arc<ToolHandle>>,
    /// `(qualified_id, embedding)` pairs for semantic search.
    embeddings: Vec<(String, Vec<f32>)>,
    /// Tracks in-flight requests for graceful shutdown.
    inflight: AtomicU64,
}

/// RAII guard that decrements the registry's in-flight request counter on drop.
#[must_use = "if unused, the in-flight request will be immediately ended"]
pub struct InflightRequestGuard<'a> {
    registry: &'a ToolRegistry,
}

impl Drop for InflightRequestGuard<'_> {
    fn drop(&mut self) {
        self.registry.end_request();
    }
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
            tools: HashMap::new(),
            embeddings: Vec::new(),
            inflight: AtomicU64::new(0),
        }
    }

    /// Loads a tool library and registers all its tools.
    ///
    /// # Errors
    ///
    /// Returns an error if loading fails or a duplicate tool ID is found.
    ///
    /// # Panics
    ///
    /// Panics if serialization of empty credentials fails.
    pub async fn load_library(
        &mut self,
        path: impl AsRef<std::path::Path>,
        checksum: Option<&str>,
        credentials: Option<&HashMap<String, HashMap<String, String>>>,
        runtime_ctx: &RuntimeContext,
    ) -> Result<(), RegistryError> {
        let library = ToolLibrary::load(&path, checksum)?;
        library.init(runtime_ctx).await?;

        self.register_module_ref(library.module(), credentials)?;
        self.libraries.push(library);

        Ok(())
    }

    /// Registers a statically linked tool module.
    ///
    /// The caller must ensure the module remains valid for the lifetime of the
    /// registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the ABI version is incompatible, initialization
    /// fails, or any tool ID is duplicated.
    pub async fn register_module(
        &mut self,
        module: ToolModuleRef,
        credentials: Option<&HashMap<String, HashMap<String, String>>>,
        runtime_ctx: &RuntimeContext,
    ) -> Result<(), RegistryError> {
        let meta = module.meta();
        if meta.abi_version != TOOL_ABI_VERSION {
            return Err(RegistryError::LoadError(LoadError::AbiMismatch {
                expected: TOOL_ABI_VERSION,
                actual: meta.abi_version,
            }));
        }

        let args = InitArgs::new(*runtime_ctx);
        let result = (module.init())(args).await;
        if result != ToolResult::Ok {
            return Err(RegistryError::LoadError(LoadError::InitFailed));
        }

        self.register_module_ref(module, credentials)
    }

    fn register_module_ref(
        &mut self,
        module: ToolModuleRef,
        credentials: Option<&HashMap<String, HashMap<String, String>>>,
    ) -> Result<(), RegistryError> {
        let meta = module.meta();
        let crate_name = meta.crate_name.as_str();
        let crate_version = meta.crate_version.as_str();

        for descriptor in module.descriptors_iter() {
            let tool_id = descriptor.id.as_str().to_string();
            let qualified_id = format!("{crate_name}.{tool_id}");

            if self.tools.contains_key(&qualified_id) {
                return Err(RegistryError::DuplicateId(qualified_id));
            }

            let info = ToolInfo {
                qualified_id: qualified_id.clone(),
                tool_id: tool_id.clone(),
                crate_name: crate_name.to_string(),
                crate_version: crate_version.to_string(),
                display_name: descriptor.name.as_str().to_string(),
                description: descriptor.description.as_str().to_string(),
                input_schema: descriptor.input_schema.as_str().to_string(),
                output_schema: descriptor.output_schema.as_str().to_string(),
                credential_schema: descriptor
                    .credential_schema
                    .as_ref()
                    .into_option()
                    .map(|s| s.as_str().to_string()),
                capabilities: descriptor
                    .capabilities
                    .as_slice()
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect(),
                tags: descriptor
                    .tags
                    .as_slice()
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect(),
                embedding: {
                    let slice = descriptor.embedding.as_slice();
                    if slice.is_empty() {
                        None
                    } else {
                        Some(slice.to_vec())
                    }
                },
            };

            if let Some(ref embedding) = info.embedding {
                self.embeddings
                    .push((qualified_id.clone(), embedding.clone()));
            }

            let system_credentials = if let Some(creds) = credentials {
                rkyv::to_bytes::<BoxedError>(creds)
                    .map_err(|e| {
                        RegistryError::LoadError(LoadError::InvalidPath(format!(
                            "serialization error: {e}",
                        )))
                    })?
                    .into_vec()
            } else {
                rkyv::to_bytes::<BoxedError>(&HashMap::<String, HashMap<String, String>>::new())
                    .expect("failed to serialize empty credentials")
                    .into_vec()
            };

            let handle = ToolHandle {
                info,
                module,
                system_credentials,
                tool_id,
            };

            info!(qualified_id = %qualified_id, "Registered tool");
            self.tools.insert(qualified_id, Arc::new(handle));
        }

        Ok(())
    }

    #[must_use]
    pub fn get(&self, qualified_id: &str) -> Option<Arc<ToolHandle>> {
        self.tools.get(qualified_id).cloned()
    }

    pub fn list(&self) -> impl Iterator<Item = &ToolInfo> {
        self.tools.values().map(|h| &h.info)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Searches tools by semantic similarity, returning results sorted by score
    /// (highest first).
    #[must_use]
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Vec<(&ToolInfo, f32)> {
        if query_embedding.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<_> = self
            .embeddings
            .iter()
            .filter_map(|(id, embedding)| {
                let score = cosine_similarity(query_embedding, embedding);
                self.tools.get(id).map(|h| (&h.info, score))
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results.truncate(limit);
        results
    }

    /// Increments the in-flight request counter.
    pub fn start_request(&self) {
        self.inflight.fetch_add(1, Ordering::Relaxed);
    }

    /// Starts a request and returns a guard that decrements the counter on
    /// drop.
    #[must_use = "dropping the guard immediately will end the request"]
    pub fn start_request_guard(&self) -> InflightRequestGuard<'_> {
        self.start_request();
        InflightRequestGuard { registry: self }
    }

    /// Decrements the in-flight request counter, saturating at zero.
    pub fn end_request(&self) {
        let _ = self
            .inflight
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(1))
            });
    }

    /// Returns the current number of in-flight requests.
    #[must_use]
    pub fn inflight_count(&self) -> u64 {
        self.inflight.load(Ordering::Relaxed)
    }

    /// Waits for all in-flight requests to complete before shutdown.
    pub async fn drain(&self) {
        while self.inflight_count() > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for (x, y) in a.iter().zip(b.iter()) {
        dot_product += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        dot_product / denominator
    }
}

#[cfg(test)]
mod tests {
    use abi_stable::{
        prefix_type::{PrefixRefTrait, WithMetadata},
        std_types::RVec,
    };
    use operai_abi::{InitArgs, ToolModule, ToolResult};

    use super::*;

    extern "C" fn test_tool_init(_args: InitArgs) -> FfiFuture<ToolResult> {
        FfiFuture::new(async { ToolResult::Ok })
    }

    extern "C" fn test_tool_call(args: CallArgs<'_>) -> FfiFuture<CallResult> {
        let tool_id = args.tool_id.as_str().to_string();
        let input = args.input.as_slice().to_vec();

        FfiFuture::new(async move {
            let mut output = tool_id.into_bytes();
            output.push(b'|');
            output.extend_from_slice(&input);
            CallResult::ok(RVec::from(output))
        })
    }

    extern "C" fn test_tool_shutdown() {}

    fn test_tool_module_ref() -> ToolModuleRef {
        let module = ToolModule {
            meta: operai_abi::ToolMeta::new(
                operai_abi::TOOL_ABI_VERSION,
                RStr::from_str("test-crate"),
                RStr::from_str("0.0.0"),
            ),
            descriptors: RSlice::from_slice(&[]),
            init: test_tool_init,
            call: test_tool_call,
            shutdown: test_tool_shutdown,
        };

        let with_metadata: &'static WithMetadata<ToolModule> =
            Box::leak(Box::new(WithMetadata::new(module)));
        ToolModuleRef::from_prefix_ref(with_metadata.static_as_prefix())
    }

    fn test_tool_info(qualified_id: &str, tool_id: &str, embedding: Option<Vec<f32>>) -> ToolInfo {
        let (crate_name, _) = qualified_id
            .split_once('.')
            .expect("qualified_id must contain '.'");

        ToolInfo {
            qualified_id: qualified_id.to_string(),
            tool_id: tool_id.to_string(),
            crate_name: crate_name.to_string(),
            crate_version: "0.0.0".to_string(),
            display_name: "Test Tool".to_string(),
            description: "Test tool description".to_string(),
            input_schema: "{}".to_string(),
            output_schema: "{}".to_string(),
            credential_schema: None,
            capabilities: Vec::new(),
            tags: Vec::new(),
            embedding,
        }
    }

    fn insert_test_tool(registry: &mut ToolRegistry, info: ToolInfo) {
        let qualified_id = info.qualified_id.clone();
        if let Some(ref embedding) = info.embedding {
            registry
                .embeddings
                .push((qualified_id.clone(), embedding.clone()));
        }

        let handle = ToolHandle {
            tool_id: info.tool_id.clone(),
            info,
            module: test_tool_module_ref(),
            system_credentials: Vec::new(),
        };

        registry.tools.insert(qualified_id, Arc::new(handle));
    }

    #[test]
    fn test_cosine_similarity_identical_vectors_returns_one() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors_returns_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors_returns_negative_one() {
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &c) + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths_returns_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors_returns_zero() {
        let empty: Vec<f32> = vec![];
        assert!(cosine_similarity(&empty, &empty).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_zero_vector_returns_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let zero = vec![0.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &zero).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_both_zero_vectors_returns_zero() {
        let zero = vec![0.0, 0.0, 0.0];
        assert!(cosine_similarity(&zero, &zero).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_inflight_counter_basic() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.inflight_count(), 0);

        registry.start_request();
        assert_eq!(registry.inflight_count(), 1);

        registry.start_request();
        assert_eq!(registry.inflight_count(), 2);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 1);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_inflight_counter_concurrent_increments() {
        use std::{sync::Arc, thread};

        let registry = Arc::new(ToolRegistry::new());
        let num_threads = 100;
        let increments_per_thread = 1000;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let registry = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..increments_per_thread {
                        registry.start_request();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(
            registry.inflight_count(),
            num_threads * increments_per_thread
        );
    }

    #[test]
    fn test_inflight_counter_concurrent_inc_dec() {
        use std::{sync::Arc, thread};

        let registry = Arc::new(ToolRegistry::new());
        let num_threads = 50;
        let ops_per_thread = 1000;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let registry = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        registry.start_request();
                        thread::yield_now();
                        registry.end_request();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(registry.inflight_count(), 0);
    }

    #[tokio::test]
    async fn test_drain_empty_registry() {
        let registry = ToolRegistry::new();
        registry.drain().await;
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_registry_error_display_messages() {
        let not_found = RegistryError::NotFound("hello-world.greet".to_string());
        assert_eq!(not_found.to_string(), "tool not found: hello-world.greet");

        let duplicate = RegistryError::DuplicateId("hello-world.greet".to_string());
        assert_eq!(
            duplicate.to_string(),
            "duplicate tool ID: hello-world.greet"
        );

        let invocation = RegistryError::InvocationError("boom".to_string());
        assert_eq!(invocation.to_string(), "tool invocation failed: boom");

        let load_error: RegistryError = LoadError::InvalidPath("bad-path".to_string()).into();
        assert_eq!(
            load_error.to_string(),
            "failed to load library: invalid path: bad-path"
        );
    }

    #[test]
    fn test_registry_default_returns_empty_registry() {
        let registry = ToolRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_search_empty_query_embedding_returns_empty() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );

        let results = registry.search(&[], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_returns_sorted_results_and_respects_limit() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", Some(vec![0.0, 1.0])),
        );

        let results = registry.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.qualified_id, "test-crate.tool-a");
        assert!((results[0].1 - 1.0).abs() < 0.0001);
        assert_eq!(results[1].0.qualified_id, "test-crate.tool-b");
        assert!((results[1].1).abs() < 0.0001);

        let limited = registry.search(&[1.0, 0.0], 1);
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].0.qualified_id, "test-crate.tool-a");
    }

    #[test]
    fn test_registry_search_skips_tools_without_embeddings() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );

        let results = registry.search(&[1.0, 0.0], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_with_limit_zero_returns_empty() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );

        let results = registry.search(&[1.0, 0.0], 0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_empty_registry_returns_empty() {
        let registry = ToolRegistry::new();
        let results = registry.search(&[1.0, 0.0], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_with_nan_embedding_does_not_panic() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", Some(vec![0.0, 1.0])),
        );

        let results = registry.search(&[f32::NAN, 0.0], 10);
        assert_eq!(results.len(), 2);

        let ids: std::collections::HashSet<_> = results
            .into_iter()
            .map(|(info, _)| info.qualified_id.as_str())
            .collect();
        assert_eq!(
            ids,
            std::collections::HashSet::from(["test-crate.tool-a", "test-crate.tool-b"])
        );
    }

    #[tokio::test]
    async fn test_tool_handle_call_passes_tool_id_and_input() {
        let module = test_tool_module_ref();
        let handle = ToolHandle {
            info: test_tool_info("test-crate.greet", "greet", None),
            module,
            system_credentials: Vec::new(),
            tool_id: "greet".to_string(),
        };

        let context = CallContext {
            request_id: RStr::from_str("request"),
            session_id: RStr::from_str("session"),
            user_id: RStr::from_str("user"),
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };
        let input = br#"{"hello":"world"}"#;

        let result = handle.call(context, RSlice::from_slice(input)).await;

        assert_eq!(result.result, ToolResult::Ok);
        assert_eq!(result.output.as_slice(), b"greet|{\"hello\":\"world\"}");
    }

    #[tokio::test]
    async fn test_drain_waits_for_inflight_requests_to_complete() {
        let registry = Arc::new(ToolRegistry::new());
        registry.start_request();

        let cloned = Arc::clone(&registry);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            cloned.end_request();
        });

        let result =
            tokio::time::timeout(tokio::time::Duration::from_secs(1), registry.drain()).await;
        assert!(result.is_ok());
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_inflight_request_guard_decrements_on_drop() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.inflight_count(), 0);

        {
            let _guard = registry.start_request_guard();
            assert_eq!(registry.inflight_count(), 1);
        }

        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_inflight_request_guard_multiple_guards() {
        let registry = ToolRegistry::new();

        let guard1 = registry.start_request_guard();
        let guard2 = registry.start_request_guard();
        assert_eq!(registry.inflight_count(), 2);

        drop(guard1);
        assert_eq!(registry.inflight_count(), 1);

        drop(guard2);
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_end_request_saturates_at_zero() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.inflight_count(), 0);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 0);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_registry_get_returns_tool_by_qualified_id() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );

        let handle = registry.get("test-crate.tool-a");
        assert!(handle.is_some());
        assert_eq!(handle.unwrap().info().qualified_id, "test-crate.tool-a");
    }

    #[test]
    fn test_registry_get_returns_none_for_nonexistent_tool() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nonexistent.tool").is_none());
    }

    #[test]
    fn test_registry_list_returns_all_tool_infos() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", None),
        );

        let infos: Vec<_> = registry.list().collect();
        assert_eq!(infos.len(), 2);

        let ids: std::collections::HashSet<_> = infos
            .iter()
            .map(|info| info.qualified_id.as_str())
            .collect();
        assert!(ids.contains("test-crate.tool-a"));
        assert!(ids.contains("test-crate.tool-b"));
    }

    #[test]
    fn test_registry_len_and_is_empty_with_tools() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", None),
        );
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_tool_handle_info_returns_tool_info() {
        let info = test_tool_info("test-crate.greet", "greet", Some(vec![1.0, 2.0]));
        let handle = ToolHandle {
            info: info.clone(),
            module: test_tool_module_ref(),
            system_credentials: Vec::new(),
            tool_id: "greet".to_string(),
        };

        let returned_info = handle.info();
        assert_eq!(returned_info.qualified_id, "test-crate.greet");
        assert_eq!(returned_info.tool_id, "greet");
        assert_eq!(returned_info.crate_name, "test-crate");
        assert_eq!(returned_info.embedding, Some(vec![1.0, 2.0]));
    }

    #[test]
    fn test_tool_info_clone_creates_independent_copy() {
        let original = test_tool_info("test-crate.greet", "greet", Some(vec![1.0, 2.0]));
        let cloned = original.clone();

        assert_eq!(original.qualified_id, cloned.qualified_id);
        assert_eq!(original.tool_id, cloned.tool_id);
        assert_eq!(original.crate_name, cloned.crate_name);
        assert_eq!(original.embedding, cloned.embedding);
    }

    #[test]
    fn test_registry_list_empty_registry_returns_empty_iterator() {
        let registry = ToolRegistry::new();
        let infos: Vec<_> = registry.list().collect();
        assert!(infos.is_empty());
    }
}
