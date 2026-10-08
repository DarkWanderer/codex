//! Server choices are independent of model refresh and restored task settings.

use super::*;
use pretty_assertions::assert_eq;

/// The collaboration mode carried by the most recent `OverrideTurnContext` in `events`.
fn last_override_collaboration_mode(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Option<CollaborationMode> {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::SubmitThreadOp {
                op:
                    AppCommand::OverrideTurnContext {
                        collaboration_mode, ..
                    },
                ..
            } => collaboration_mode,
            _ => None,
        })
        .last()
}

#[tokio::test]
async fn mode_switch_keeps_per_mode_session_choices() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("implementation-model")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_plan_mode_model(Some("plan-model".into()));
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::High));
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Low));

    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "plan-model");
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
    let sent = last_override_collaboration_mode(&mut events).expect("override turn context");
    assert_eq!(sent.mode, ModeKind::Plan);
    assert_eq!(sent.settings.model, "plan-model");
    assert_eq!(
        sent.settings.reasoning_effort,
        Some(ReasoningEffortConfig::High)
    );

    // A session pick in Plan must not leak into Default, and must survive a round trip.
    chat.set_model("session-plan-model");
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::Ultra));
    assert_eq!(
        chat.current_collaboration_mode().model(),
        "implementation-model"
    );

    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "implementation-model");
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::Low)
    );
    let sent = last_override_collaboration_mode(&mut events).expect("override turn context");
    assert_eq!(sent.mode, ModeKind::Default);
    assert_eq!(sent.settings.model, "implementation-model");
    assert_eq!(
        sent.settings.reasoning_effort,
        Some(ReasoningEffortConfig::Low)
    );

    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "session-plan-model");
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::Ultra)
    );
    let sent = last_override_collaboration_mode(&mut events).expect("override turn context");
    assert_eq!(sent.mode, ModeKind::Plan);
    assert_eq!(sent.settings.model, "session-plan-model");
}

#[tokio::test]
async fn default_session_model_survives_mode_switch() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("implementation-model")).await;
    chat.set_plan_mode_model(Some("plan-model".into()));
    chat.set_model("session-implementation-model");

    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "plan-model");
    chat.cycle_collaboration_mode();

    assert_eq!(chat.current_model(), "session-implementation-model");
}

#[tokio::test]
async fn unset_plan_model_inherits_default_model() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("implementation-model")).await;
    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "implementation-model");
    let plan_effort = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .and_then(|mask| mask.reasoning_effort)
        .flatten();
    assert_eq!(chat.effective_reasoning_effort(), plan_effort);

    // A Default session change carries into Plan while Plan has no model of its own.
    chat.cycle_collaboration_mode();
    chat.set_model("session-implementation-model");
    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "session-implementation-model");

    // The same change made while Plan is active reaches Plan immediately.
    chat.set_default_mode_model("later-implementation-model");
    assert_eq!(chat.current_model(), "later-implementation-model");

    // Once Plan has its own model, Default changes no longer reach it.
    chat.set_plan_mode_model(Some("plan-model".into()));
    chat.set_default_mode_model("final-implementation-model");
    assert_eq!(chat.current_model(), "plan-model");
    chat.cycle_collaboration_mode();
    assert_eq!(chat.current_model(), "final-implementation-model");
}

#[tokio::test]
async fn collaboration_catalog_selection_clears_restored_prompt_and_survives_model_refresh() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.2")).await;
    let restored = CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: "gpt-5.2".into(),
            reasoning_effort: Some(ReasoningEffortConfig::Low),
            developer_instructions: Some("stale restored prompt".into()),
        },
    };
    chat.set_effective_collaboration_mode(restored.clone());
    let plan = CollaborationModeMask {
        name: "Server plan".into(),
        mode: Some(ModeKind::Plan),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffortConfig::High)),
        developer_instructions: Some(None),
    };
    Arc::make_mut(&mut chat.model_catalog).collaboration_modes = vec![plan.clone()];
    let mut models = chat.model_catalog.try_list_models().unwrap();
    models[0].description = "Refreshed model description".into();
    let request_id = uuid::Uuid::new_v4();
    chat.model_popup_request_id = Some(request_id);
    assert!(chat.on_models_loaded(request_id, Ok(models)));
    assert_eq!(chat.model_catalog.collaboration_modes, vec![plan]);
    assert_eq!(chat.effective_collaboration_mode(), restored);
    assert!(collaboration_modes::default_mode_mask(&chat.model_catalog).is_none());

    chat.cycle_collaboration_mode();
    assert_eq!(
        chat.effective_collaboration_mode(),
        CollaborationMode {
            mode: ModeKind::Plan,
            settings: Settings {
                model: "gpt-5.2".into(),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                developer_instructions: None,
            },
        }
    );
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::Ultra));
    chat.cycle_collaboration_mode();
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::Ultra)
    );
    chat.set_plan_mode_reasoning_effort(/*effort*/ None);
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
}

#[tokio::test]
async fn collaboration_catalog_unavailable_preserves_task_and_plan_input() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.2")).await;
    Arc::make_mut(&mut chat.model_catalog)
        .collaboration_modes
        .clear();
    for mode in [ModeKind::Default, ModeKind::Plan] {
        let restored = CollaborationMode {
            mode,
            settings: Settings {
                model: "task-model".into(),
                reasoning_effort: Some(ReasoningEffortConfig::Ultra),
                developer_instructions: Some("restored prompt".into()),
            },
        };
        chat.set_effective_collaboration_mode(restored.clone());
        chat.cycle_collaboration_mode();
        assert_eq!(chat.effective_collaboration_mode(), restored);
    }
    chat.set_effective_collaboration_mode(CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: "task-model".into(),
            reasoning_effort: Some(ReasoningEffortConfig::Low),
            developer_instructions: None,
        },
    });
    chat.thread_id = Some(ThreadId::new());
    chat.bottom_pane
        .set_composer_text("/plan keep my draft".into(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_eq!(chat.bottom_pane.composer_text(), "/plan keep my draft");
    assert_eq!(chat.active_mode_kind(), ModeKind::Default);
}
