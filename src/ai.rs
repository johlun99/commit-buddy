use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::git::DiffInfo;
use crate::config::Config;
use async_openai::{
    Client,
    types::{
        ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequestArgs,
    },
    config::OpenAIConfig,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub confidence: Option<f32>,
}

pub async fn call_openai_api(system_prompt: &str, user_prompt: &str, config: &Config) -> Result<String> {
    if !config.has_openai_key() {
        return Ok(format!(
            "🤖 AI Feature Unavailable\n\n{}\n\n*Note: Set OPENAI_API_KEY environment variable to enable AI features.*",
            user_prompt
        ));
    }

    let api_key = config.openai_api_key.as_ref().unwrap();
    let client = Client::with_config(OpenAIConfig::new().with_api_key(api_key));

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4o-mini")
        .messages(vec![
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: system_prompt.to_string(),
                name: None,
            }),
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(user_prompt.to_string()),
                name: None,
            }),
        ])
        .max_tokens(2000u16)
        .temperature(0.7)
        .build()?;

        let response = client.chat().create(request).await?;

    let content = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "⚠️ Empty response from model".to_string());

    Ok(content)
}

pub async fn generate_pr_description(diff_info: &DiffInfo, config: &Config) -> Result<String> {
    let commits_summary = diff_info.commits.iter()
        .map(|c| format!("- {}: {}", &c.hash[..8], c.message))
        .collect::<Vec<_>>()
        .join("\n");
    
    let files_summary = diff_info.commits.iter()
        .flat_map(|c| &c.files_changed)
        .collect::<std::collections::HashSet<_>>()
        .iter()
        .map(|f| format!("- {}", f))
        .collect::<Vec<_>>()
        .join("\n");
    
    let system_prompt = "You are an expert software engineer creating a pull request description. Generate a comprehensive PR description in markdown format that includes a clear title, summary of changes, what was modified and why, any breaking changes, testing instructions, and screenshots if relevant.";
    
    let user_prompt = format!(
        "Based on the following commit information, generate a comprehensive PR description:\n\nCommits:\n{}\n\nFiles changed:\n{}\n\nTotal files changed: {}\n\nPlease create a professional PR description with proper markdown formatting.",
        commits_summary,
        files_summary,
        diff_info.total_files_changed
    );
    
    call_openai_api(system_prompt, &user_prompt, config).await
}

pub async fn generate_tests(diff_info: &DiffInfo, framework: &str, config: &Config) -> Result<String> {
    let code_changes = diff_info.commits.iter()
        .map(|c| format!("Commit {}: {}\nDiff:\n{}", &c.hash[..8], c.message, c.diff))
        .collect::<Vec<_>>()
        .join("\n\n");
    
    let system_prompt = "You are an expert software engineer writing comprehensive unit tests. Generate well-structured unit tests with proper test cases, edge cases, error handling, and mocking for external dependencies.";
    
    let user_prompt = format!(
        "Based on the following code changes, generate comprehensive unit tests using the {} framework:\n\n{}\n\nPlease generate:\n1. Unit tests for all new/modified functions\n2. Edge cases and error handling tests\n3. Integration tests if applicable\n4. Mocking for external dependencies\n5. Clear test descriptions and assertions\n\nFormat the tests as proper code blocks with syntax highlighting.",
        framework,
        code_changes
    );
    
    call_openai_api(system_prompt, &user_prompt, config).await
}

pub async fn improve_commit_message(message: &str, config: &Config) -> Result<String> {
    let system_prompt = "You are an expert software engineer helping to improve commit messages. Provide an improved version that follows conventional commit format with imperative mood, clear subject line, and proper body if needed.";
    
    let user_prompt = format!(
        "The current commit message is: \"{}\"\n\nPlease provide an improved version that follows conventional commit format:\n- Use imperative mood (\"Add feature\" not \"Added feature\")\n- Keep the subject line under 50 characters\n- Use the body to explain what and why, not how\n- Reference issues if applicable\n\nProvide only the improved commit message, no additional commentary.",
        message
    );
    
    call_openai_api(system_prompt, &user_prompt, config).await
}

pub async fn generate_commit_suggestions(diff_info: &DiffInfo, config: &Config) -> Result<Vec<String>> {
    let staged_changes = diff_info.commits.iter()
        .map(|c| format!("Files: {}\nDiff:\n{}", c.files_changed.join(", "), c.diff))
        .collect::<Vec<_>>()
        .join("\n\n");
    
    let system_prompt = "You are an expert software engineer helping to write commit messages. Suggest 3 different commit messages following conventional commit format.";
    
    let user_prompt = format!(
        "Based on the following staged changes, suggest 3 different commit messages:\n\n{}\n\nProvide 3 options:\n1. A concise, single-line commit message\n2. A more descriptive commit message with body\n3. A detailed commit message with multiple paragraphs if needed\n\nEach suggestion should follow conventional commit format (feat, fix, docs, style, refactor, test, chore).",
        staged_changes
    );
    
    let response = call_openai_api(system_prompt, &user_prompt, config).await?;
    
    let suggestions: Vec<String> = response.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect();
    
    Ok(suggestions)
}

pub async fn generate_changelog(diff_info: &DiffInfo, config: &Config) -> Result<String> {
    let commits_summary = diff_info.commits.iter()
        .map(|c| format!("- {}: {}", &c.hash[..8], c.message))
        .collect::<Vec<_>>()
        .join("\n");
    
    let system_prompt = "You are an expert software engineer creating a changelog. Generate a professional changelog in markdown format following Keep a Changelog standards.";
    
    let user_prompt = format!(
        "Based on the following commits, generate a professional changelog:\n\n{}\n\nPlease create a changelog that includes:\n1. A clear version header\n2. Categorized changes (Added, Changed, Fixed, Removed, etc.)\n3. Breaking changes section if applicable\n4. Contributors if available\n5. Links to issues/PRs if mentioned in commits\n\nFormat as proper markdown following Keep a Changelog format.",
        commits_summary
    );
    
    call_openai_api(system_prompt, &user_prompt, config).await
}

pub async fn code_review(diff_info: &DiffInfo, config: &Config) -> Result<String> {
    let code_changes = diff_info.commits.iter()
        .map(|c| format!("Commit {}: {}\nDiff:\n{}", &c.hash[..8], c.message, c.diff))
        .collect::<Vec<_>>()
        .join("\n\n");
    
    let system_prompt = "You are an expert software engineer performing a code review. Provide comprehensive feedback on code quality, potential bugs, performance, security, maintainability, and testing.";
    
    let user_prompt = format!(
        "Please review the following code changes and provide feedback:\n\n{}\n\nPlease review and provide feedback on:\n1. Code quality and best practices\n2. Potential bugs or issues\n3. Performance considerations\n4. Security concerns\n5. Maintainability and readability\n6. Testing coverage\n7. Documentation needs\n\nFormat your review as constructive feedback with specific suggestions for improvement.",
        code_changes
    );
    
    call_openai_api(system_prompt, &user_prompt, config).await
}