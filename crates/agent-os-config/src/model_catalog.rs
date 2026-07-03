use agent_os_sys::ModelCapabilities;

pub(crate) fn default_model_capabilities(
    provider_id: &str,
    model_id: &str,
) -> Option<ModelCapabilities> {
    let provider = provider_id.trim().to_ascii_lowercase();
    let model = model_id.trim().to_ascii_lowercase();

    match (provider.as_str(), model.as_str()) {
        ("openai", "gpt-4o") => Some(openai_multimodal()),
        ("openai", "gpt-4o-mini") => Some(openai_multimodal()),
        ("anthropic", "claude-sonnet-4-20250514") => Some(anthropic_multimodal()),
        ("tongyi", "qwen3.6-plus") => Some(openai_chat_completions_multimodal()),
        ("zhipuai", "glm-5.2") => Some(openai_chat_completions_text_only()),
        ("xiaomi", "mimo-v2.5-pro") => Some(openai_chat_completions_text_only()),
        _ => None,
    }
}

fn openai_multimodal() -> ModelCapabilities {
    ModelCapabilities {
        streaming: true,
        tool_calling: true,
        reasoning: true,
        temperature: true,
        image_input: true,
        structured_output: true,
    }
}

fn anthropic_multimodal() -> ModelCapabilities {
    ModelCapabilities {
        streaming: true,
        tool_calling: true,
        reasoning: true,
        image_input: true,
        ..ModelCapabilities::default()
    }
}

fn openai_chat_completions_multimodal() -> ModelCapabilities {
    ModelCapabilities {
        streaming: true,
        tool_calling: true,
        reasoning: true,
        temperature: true,
        image_input: true,
        structured_output: true,
    }
}

fn openai_chat_completions_text_only() -> ModelCapabilities {
    ModelCapabilities {
        streaming: true,
        tool_calling: true,
        reasoning: true,
        temperature: true,
        structured_output: true,
        ..ModelCapabilities::default()
    }
}
