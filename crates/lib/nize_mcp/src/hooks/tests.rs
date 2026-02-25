// @awa-component: MCP-HookTests

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use async_trait::async_trait;

    use crate::hooks::{
        HookContext, HookError, HookPipeline, HookScope, ToolCallOutcome, ToolHook,
    };

    /// Test hook that records call order.
    struct OrderTracker {
        name: String,
        before_counter: Arc<AtomicU32>,
        after_counter: Arc<AtomicU32>,
        before_order: Arc<std::sync::Mutex<Vec<String>>>,
        after_order: Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ToolHook for OrderTracker {
        async fn before_call(
            &self,
            _ctx: &HookContext,
            _params: &mut serde_json::Value,
        ) -> Result<(), HookError> {
            self.before_counter.fetch_add(1, Ordering::SeqCst);
            self.before_order.lock().unwrap().push(self.name.clone());
            Ok(())
        }

        async fn after_call(
            &self,
            _ctx: &HookContext,
            _outcome: &mut ToolCallOutcome,
        ) -> Result<(), HookError> {
            self.after_counter.fetch_add(1, Ordering::SeqCst);
            self.after_order.lock().unwrap().push(self.name.clone());
            Ok(())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    /// Test hook that rejects in before_call.
    struct RejectHook;

    #[async_trait]
    impl ToolHook for RejectHook {
        async fn before_call(
            &self,
            _ctx: &HookContext,
            _params: &mut serde_json::Value,
        ) -> Result<(), HookError> {
            Err(HookError::AccessDenied("rejected".into()))
        }

        async fn after_call(
            &self,
            _ctx: &HookContext,
            _outcome: &mut ToolCallOutcome,
        ) -> Result<(), HookError> {
            Ok(())
        }

        fn name(&self) -> &str {
            "RejectHook"
        }
    }

    fn make_ctx() -> HookContext {
        HookContext {
            user_id: "user-1".to_string(),
            server_id: None,
            tool_name: "test_tool".to_string(),
            tool_id: None,
            scope: HookScope::Global,
            timestamp: chrono::Utc::now(),
        }
    }

    // @awa-test: PLAN-024 — pipeline runs before_call in order
    #[tokio::test]
    async fn pipeline_before_call_order() {
        let before_order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let after_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let hooks: Vec<(HookScope, Arc<dyn ToolHook>)> = vec![
            (
                HookScope::Global,
                Arc::new(OrderTracker {
                    name: "A".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: Arc::new(AtomicU32::new(0)),
                    before_order: before_order.clone(),
                    after_order: after_order.clone(),
                }),
            ),
            (
                HookScope::Global,
                Arc::new(OrderTracker {
                    name: "B".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: Arc::new(AtomicU32::new(0)),
                    before_order: before_order.clone(),
                    after_order: after_order.clone(),
                }),
            ),
        ];

        let pipeline = HookPipeline::new(hooks);
        let ctx = make_ctx();
        let mut params = serde_json::json!({});

        pipeline.run_before(&ctx, &mut params).await.unwrap();

        let order = before_order.lock().unwrap();
        assert_eq!(*order, vec!["A", "B"]);
    }

    // @awa-test: PLAN-024 — pipeline runs after_call in reverse order
    #[tokio::test]
    async fn pipeline_after_call_reverse_order() {
        let before_order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let after_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let hooks: Vec<(HookScope, Arc<dyn ToolHook>)> = vec![
            (
                HookScope::Global,
                Arc::new(OrderTracker {
                    name: "A".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: Arc::new(AtomicU32::new(0)),
                    before_order: before_order.clone(),
                    after_order: after_order.clone(),
                }),
            ),
            (
                HookScope::Global,
                Arc::new(OrderTracker {
                    name: "B".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: Arc::new(AtomicU32::new(0)),
                    before_order: before_order.clone(),
                    after_order: after_order.clone(),
                }),
            ),
        ];

        let pipeline = HookPipeline::new(hooks);
        let ctx = make_ctx();
        let mut outcome = ToolCallOutcome::Success(serde_json::json!({}));

        pipeline.run_after(&ctx, &mut outcome).await.unwrap();

        let order = after_order.lock().unwrap();
        assert_eq!(*order, vec!["B", "A"]);
    }

    // @awa-test: PLAN-024 — pipeline short-circuits on before_call error
    #[tokio::test]
    async fn pipeline_short_circuits_on_error() {
        let after_counter = Arc::new(AtomicU32::new(0));
        let before_order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let after_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let hooks: Vec<(HookScope, Arc<dyn ToolHook>)> = vec![
            (HookScope::Global, Arc::new(RejectHook)),
            (
                HookScope::Global,
                Arc::new(OrderTracker {
                    name: "B".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: after_counter.clone(),
                    before_order,
                    after_order,
                }),
            ),
        ];

        let pipeline = HookPipeline::new(hooks);
        let ctx = make_ctx();
        let mut params = serde_json::json!({});

        let result = pipeline.run_before(&ctx, &mut params).await;
        assert!(result.is_err());
        // B's before_call should not have been called
        assert_eq!(after_counter.load(Ordering::SeqCst), 0);
    }

    // @awa-test: PLAN-024 — empty pipeline is no-op
    #[tokio::test]
    async fn empty_pipeline_is_noop() {
        let pipeline = HookPipeline::empty();
        let ctx = make_ctx();
        let mut params = serde_json::json!({});
        let mut outcome = ToolCallOutcome::Success(serde_json::json!({}));

        pipeline.run_before(&ctx, &mut params).await.unwrap();
        pipeline.run_after(&ctx, &mut outcome).await.unwrap();
    }

    // @awa-test: PLAN-024 — scope filtering works correctly
    #[tokio::test]
    async fn scope_filtering() {
        let before_order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let after_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let server_id = uuid::Uuid::new_v4();

        let hooks: Vec<(HookScope, Arc<dyn ToolHook>)> = vec![
            (
                HookScope::Global,
                Arc::new(OrderTracker {
                    name: "global".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: Arc::new(AtomicU32::new(0)),
                    before_order: before_order.clone(),
                    after_order: after_order.clone(),
                }),
            ),
            (
                HookScope::Server(server_id),
                Arc::new(OrderTracker {
                    name: "server-specific".into(),
                    before_counter: Arc::new(AtomicU32::new(0)),
                    after_counter: Arc::new(AtomicU32::new(0)),
                    before_order: before_order.clone(),
                    after_order: after_order.clone(),
                }),
            ),
        ];

        let pipeline = HookPipeline::new(hooks);

        // Call with no server_id — only global hook should run
        let ctx = make_ctx();
        let mut params = serde_json::json!({});
        pipeline.run_before(&ctx, &mut params).await.unwrap();

        let order = before_order.lock().unwrap().clone();
        assert_eq!(order, vec!["global"]);
        drop(order);
        before_order.lock().unwrap().clear();

        // Call with matching server_id — both should run
        let ctx2 = HookContext {
            server_id: Some(server_id),
            ..make_ctx()
        };
        pipeline.run_before(&ctx2, &mut params).await.unwrap();

        let order = before_order.lock().unwrap().clone();
        assert_eq!(order, vec!["global", "server-specific"]);
    }
}
