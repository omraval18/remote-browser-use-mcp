use browser_use_core::ProviderBackend;
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum AgentBackend {
    Codex,
    Openai,
    Anthropic,
    Openrouter,
    Deepseek,
    Ollama,
    Fake,
    None,
}

impl AgentBackend {
    pub(crate) fn as_setting(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::Deepseek => "deepseek",
            Self::Ollama => "ollama",
            Self::Fake => "fake",
            Self::None => "none",
        }
    }

    pub(crate) fn from_setting(value: &str) -> Option<Self> {
        match value {
            "codex" => Some(Self::Codex),
            "openai" => Some(Self::Openai),
            "anthropic" => Some(Self::Anthropic),
            "openrouter" => Some(Self::Openrouter),
            "deepseek" => Some(Self::Deepseek),
            "ollama" => Some(Self::Ollama),
            "fake" => Some(Self::Fake),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

impl From<AgentBackend> for ProviderBackend {
    fn from(value: AgentBackend) -> Self {
        match value {
            AgentBackend::Codex => Self::Codex,
            AgentBackend::Openai => Self::Openai,
            AgentBackend::Anthropic => Self::Anthropic,
            AgentBackend::Openrouter => Self::Openrouter,
            AgentBackend::Deepseek => Self::Deepseek,
            AgentBackend::Ollama => Self::Ollama,
            AgentBackend::Fake => Self::Fake,
            AgentBackend::None => Self::None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ModelChoice {
    pub(crate) display: &'static str,
    pub(crate) account: &'static str,
    pub(crate) backend: AgentBackend,
    pub(crate) provider_model: &'static str,
}

pub(crate) const ACCOUNT_CODEX: &str = "Codex login";
pub(crate) const ACCOUNT_CLAUDE_CODE: &str = "Claude Code subscription";
pub(crate) const ACCOUNT_CLAUDE_CODE_LEGACY: &str = "Claude Code login";
pub(crate) const ACCOUNT_OPENAI: &str = "OpenAI API key";
pub(crate) const ACCOUNT_ANTHROPIC: &str = "Anthropic API key";
pub(crate) const ACCOUNT_OPENROUTER: &str = "OpenRouter API key";
pub(crate) const ACCOUNT_DEEPSEEK: &str = "DeepSeek API key";
pub(crate) const ACCOUNT_OLLAMA: &str = "Ollama (local)";

pub(crate) const ACCOUNT_CHOICES: [&str; 6] = [
    ACCOUNT_CODEX,
    ACCOUNT_OPENAI,
    ACCOUNT_ANTHROPIC,
    ACCOUNT_OPENROUTER,
    ACCOUNT_DEEPSEEK,
    ACCOUNT_OLLAMA,
];

pub(crate) const BROWSER_USE_CLOUD: &str = "Browser Use cloud";
pub(crate) const BROWSER_USE_CLOUD_API_KEY_SETTING: &str = "auth.browser_use_cloud.api_key";
pub(crate) const BROWSER_USE_CLOUD_API_KEY_ENV: &str = "BROWSER_USE_API_KEY";
pub(crate) const BROWSER_LOCAL_CHROME: &str = "Local Chrome";
pub(crate) const BROWSER_CHOICES: [&str; 3] =
    [BROWSER_LOCAL_CHROME, BROWSER_USE_CLOUD, "Headless Chromium"];

pub(crate) fn browser_use_cloud_env_key_present() -> bool {
    std::env::var(BROWSER_USE_CLOUD_API_KEY_ENV).is_ok_and(|value| !value.trim().is_empty())
}

pub(crate) fn is_claude_code_account(account: &str) -> bool {
    account == ACCOUNT_CLAUDE_CODE || account == ACCOUNT_CLAUDE_CODE_LEGACY
}

pub(crate) const MODEL_CHOICES: [ModelChoice; 49] = [
    ModelChoice {
        display: "GPT-5.5",
        account: ACCOUNT_CODEX,
        backend: AgentBackend::Codex,
        provider_model: "gpt-5.5",
    },
    ModelChoice {
        display: "Claude Sonnet 4.6",
        account: ACCOUNT_CLAUDE_CODE,
        backend: AgentBackend::Anthropic,
        provider_model: "claude-sonnet-4-6",
    },
    ModelChoice {
        display: "Claude Opus 4.7",
        account: ACCOUNT_CLAUDE_CODE,
        backend: AgentBackend::Anthropic,
        provider_model: "claude-opus-4-7",
    },
    ModelChoice {
        display: "GPT-5.5",
        account: ACCOUNT_OPENAI,
        backend: AgentBackend::Openai,
        provider_model: "gpt-5.5",
    },
    ModelChoice {
        display: "Claude Sonnet 4.6",
        account: ACCOUNT_ANTHROPIC,
        backend: AgentBackend::Anthropic,
        provider_model: "claude-sonnet-4-6",
    },
    ModelChoice {
        display: "Claude Opus 4.7",
        account: ACCOUNT_ANTHROPIC,
        backend: AgentBackend::Anthropic,
        provider_model: "claude-opus-4-7",
    },
    ModelChoice {
        display: "Qwen3.6 Plus",
        account: ACCOUNT_OPENROUTER,
        backend: AgentBackend::Openrouter,
        provider_model: "qwen/qwen3.6-plus",
    },
    ModelChoice {
        display: "Kimi K2.5",
        account: ACCOUNT_OPENROUTER,
        backend: AgentBackend::Openrouter,
        provider_model: "moonshotai/kimi-k2.5",
    },
    ModelChoice {
        display: "DeepSeek V4 Pro",
        account: ACCOUNT_OPENROUTER,
        backend: AgentBackend::Openrouter,
        provider_model: "deepseek/deepseek-v4-pro",
    },
    ModelChoice {
        display: "DeepSeek V4 Pro",
        account: ACCOUNT_DEEPSEEK,
        backend: AgentBackend::Deepseek,
        provider_model: "deepseek-v4-pro",
    },
    // Ollama local models (indices 10-48)
    ModelChoice {
        display: "Cogito 2.1 671B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "cogito-2.1:671b",
    },
    ModelChoice {
        display: "GLM 4.7",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "glm-4.7",
    },
    ModelChoice {
        display: "GLM 5.1",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "glm-5.1",
    },
    ModelChoice {
        display: "Qwen3 Coder 480B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "qwen3-coder:480b",
    },
    ModelChoice {
        display: "DeepSeek V3.2",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "deepseek-v3.2",
    },
    ModelChoice {
        display: "DeepSeek V3.1 671B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "deepseek-v3.1:671b",
    },
    ModelChoice {
        display: "Ministral 3 3B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "ministral-3:3b",
    },
    ModelChoice {
        display: "Ministral 3 14B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "ministral-3:14b",
    },
    ModelChoice {
        display: "Kimi K2 Thinking",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "kimi-k2-thinking",
    },
    ModelChoice {
        display: "MiniMax M2.7",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "minimax-m2.7",
    },
    ModelChoice {
        display: "Devstral Small 2 24B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "devstral-small-2:24b",
    },
    ModelChoice {
        display: "Gemini 3 Flash Preview",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gemini-3-flash-preview",
    },
    ModelChoice {
        display: "Gemma3 4B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gemma3:4b",
    },
    ModelChoice {
        display: "Gemma3 27B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gemma3:27b",
    },
    ModelChoice {
        display: "DeepSeek V4 Flash",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "deepseek-v4-flash",
    },
    ModelChoice {
        display: "Nemotron 3 Nano 30B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "nemotron-3-nano:30b",
    },
    ModelChoice {
        display: "Qwen3 Next 80B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "qwen3-next:80b",
    },
    ModelChoice {
        display: "Qwen3 Coder Next",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "qwen3-coder-next",
    },
    ModelChoice {
        display: "GPT OSS 20B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gpt-oss:20b",
    },
    ModelChoice {
        display: "Mistral Large 3 675B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "mistral-large-3:675b",
    },
    ModelChoice {
        display: "Kimi K2 1T",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "kimi-k2:1t",
    },
    ModelChoice {
        display: "Kimi K2.5",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "kimi-k2.5",
    },
    ModelChoice {
        display: "Qwen3 VL 235B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "qwen3-vl:235b",
    },
    ModelChoice {
        display: "MiniMax M2.5",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "minimax-m2.5",
    },
    ModelChoice {
        display: "Gemma3 12B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gemma3:12b",
    },
    ModelChoice {
        display: "Gemma4 31B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gemma4:31b",
    },
    ModelChoice {
        display: "RNJ 1 8B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "rnj-1:8b",
    },
    ModelChoice {
        display: "GLM 4.6",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "glm-4.6",
    },
    ModelChoice {
        display: "Kimi K2.6",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "kimi-k2.6",
    },
    ModelChoice {
        display: "DeepSeek V4 Pro (Ollama)",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "deepseek-v4-pro",
    },
    ModelChoice {
        display: "GPT OSS 120B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "gpt-oss:120b",
    },
    ModelChoice {
        display: "MiniMax M2",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "minimax-m2",
    },
    ModelChoice {
        display: "Ministral 3 8B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "ministral-3:8b",
    },
    ModelChoice {
        display: "Devstral 2 123B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "devstral-2:123b",
    },
    ModelChoice {
        display: "Qwen3.5 397B",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "qwen3.5:397b",
    },
    ModelChoice {
        display: "GLM 5",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "glm-5",
    },
    ModelChoice {
        display: "Qwen3 VL 235B Instruct",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "qwen3-vl:235b-instruct",
    },
    ModelChoice {
        display: "MiniMax M2.1",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "minimax-m2.1",
    },
    ModelChoice {
        display: "Nemotron 3 Super",
        account: ACCOUNT_OLLAMA,
        backend: AgentBackend::Ollama,
        provider_model: "nemotron-3-super",
    },
];

pub(crate) const VISIBLE_MODEL_CHOICES: [usize; 47] = [
    0, 3, 4, 5, 6, 7, 8, 9, // Ollama models
    10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33,
    34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
];

pub(crate) fn provider_model_for_display(display: &str) -> &str {
    MODEL_CHOICES
        .iter()
        .find(|choice| choice.display == display)
        .map(|choice| choice.provider_model)
        .unwrap_or(display)
}
