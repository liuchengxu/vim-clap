#![allow(dead_code)]
use types::AutocmdEventType;

#[async_trait::async_trait]
trait Plugin {
    fn subscriptions(&self) -> &[AutocmdEventType] {
        &[]
    }

    async fn handle_autocmd(&self, event_type: AutocmdEventType);
}

struct TestSubscriptions;

#[async_trait::async_trait]
impl Plugin for TestSubscriptions {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&self, event_type: AutocmdEventType) {
        use AutocmdEventType::{BufEnter, BufLeave, CursorMoved, InsertEnter};

        match event_type {
            BufEnter | BufLeave => {}
            CursorMoved => {}
            InsertEnter if true => {}
            _unknown => {}
        }
    }
}

struct NoSubscriptions;

#[async_trait::async_trait]
impl Plugin for NoSubscriptions {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&self, _event_type: AutocmdEventType) {}
}

#[test]
fn test_subscriptions_macro() {
    assert_eq!(
        TestSubscriptions.subscriptions(),
        &[
            AutocmdEventType::BufEnter,
            AutocmdEventType::BufLeave,
            AutocmdEventType::CursorMoved,
            AutocmdEventType::InsertEnter
        ]
    );

    assert_eq!(NoSubscriptions.subscriptions(), &[]);
}
