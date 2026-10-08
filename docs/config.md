# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

To give Plan and implementation mode separate model and reasoning defaults in the terminal UI:

```toml
model = "gpt-6.1-sol"
model_reasoning_effort = "medium"
plan_mode_model = "gpt-6-astra"
plan_mode_reasoning_effort = "high"
```

When `plan_mode_model` is unset, Plan uses the implementation model. In Plan mode,
choosing a model or reasoning level in `/model` asks whether to apply it to Plan
mode only or to the global default as well. See the configuration reference
linked above for general config behavior and the default Plan reasoning effort.

## Lifecycle hooks

Admins can set top-level `allow_managed_hooks_only = true` in
`requirements.toml` to ignore user, project, and session hook configs while
still allowing managed hooks from requirements and managed config layers. This
setting is only supported in `requirements.toml`; putting it in `config.toml`
does not enable managed-hooks-only mode.
