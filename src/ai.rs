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
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub confidence: Option<f32>,
}

#[derive(Debug)]
struct ProjectInfo {
    project_type: String,
    test_framework: String,
    test_directory: String,
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

pub async fn generate_tests(diff_info: &DiffInfo, _framework: &str, config: &Config) -> Result<String> {
    let code_changes = diff_info.commits.iter()
        .map(|c| format!("Commit {}: {}\nDiff:\n{}", &c.hash[..8], c.message, c.diff))
        .collect::<Vec<_>>()
        .join("\n\n");
    
    // Detect project type and determine appropriate test framework and directory structure
    let project_info = detect_project_type(&diff_info);
    
    let system_prompt = "You are an expert software engineer writing comprehensive unit tests. Generate well-structured unit tests that will actually compile and run. Focus on testing the actual functions that exist in the codebase. Return ONLY the test code without any markdown formatting, explanations, or additional text.";
    
    let user_prompt = format!(
        "Based on the following code changes, generate working unit tests for a {} project using {}:\n\n{}\n\nIMPORTANT: Generate tests that will actually compile and run. Only test functions that actually exist in the codebase. For Rust projects:\n- Use proper module imports (e.g., use commit_buddy::ai::*)\n- Don't use external crates that aren't in Cargo.toml\n- Focus on testing the actual functions: generate_pr_description, generate_tests, improve_commit_message, generate_changelog, code_review\n- Use simple assertions without complex mocking\n- Make sure all imports are correct\n\nReturn only the raw test code, no explanations or markdown.",
        project_info.project_type,
        project_info.test_framework,
        code_changes
    );
    
    let test_content = call_openai_api(system_prompt, &user_prompt, config).await?;
    
    // Create the test directory if it doesn't exist
    let test_dir = Path::new(&project_info.test_directory);
    if !test_dir.exists() {
        fs::create_dir_all(test_dir)?;
        println!("📁 Created test directory: {}", project_info.test_directory);
    }
    
    // Generate test files based on project type
    match project_info.project_type.as_str() {
        "Rust" => {
            create_rust_tests(&test_content, test_dir)?;
        }
        "Python" => {
            create_python_tests(&test_content, test_dir)?;
        }
        "JavaScript/TypeScript" => {
            create_js_tests(&test_content, test_dir)?;
        }
        "Java" => {
            create_java_tests(&test_content, test_dir)?;
        }
        "Go" => {
            create_go_tests(&test_content, test_dir)?;
        }
        "C/C++" => {
            create_cpp_tests(&test_content, test_dir)?;
        }
        "C#" => {
            create_csharp_tests(&test_content, test_dir)?;
        }
        _ => {
            create_generic_tests(&test_content, test_dir)?;
        }
    }
    
    Ok(format!("✅ Tests generated successfully in {} directory using {} framework!", 
               project_info.test_directory, project_info.test_framework))
}

fn detect_project_type(diff_info: &DiffInfo) -> ProjectInfo {
    let mut project_type = "Unknown";
    let mut test_framework = "generic";
    let mut test_directory = "tests/";
    
    // Analyze file extensions and patterns to detect project type
    let file_extensions: std::collections::HashSet<String> = diff_info.commits.iter()
        .flat_map(|c| &c.files_changed)
        .map(|f| {
            f.split('.')
                .last()
                .unwrap_or("")
                .to_lowercase()
        })
        .collect();
    
    // Check for Rust project
    if file_extensions.contains("rs") || diff_info.commits.iter().any(|c| c.files_changed.iter().any(|f| f.contains("Cargo.toml"))) {
        project_type = "Rust";
        test_framework = "cargo test";
        test_directory = "tests/";
    }
    // Check for Python project
    else if file_extensions.contains("py") || diff_info.commits.iter().any(|c| c.files_changed.iter().any(|f| f.contains("requirements.txt") || f.contains("setup.py") || f.contains("pyproject.toml"))) {
        project_type = "Python";
        test_framework = "pytest";
        test_directory = "tests/";
    }
    // Check for JavaScript/Node.js project
    else if file_extensions.contains("js") || file_extensions.contains("ts") || diff_info.commits.iter().any(|c| c.files_changed.iter().any(|f| f.contains("package.json"))) {
        project_type = "JavaScript/TypeScript";
        test_framework = "jest";
        test_directory = "__tests__/ or tests/";
    }
    // Check for Java project
    else if file_extensions.contains("java") || diff_info.commits.iter().any(|c| c.files_changed.iter().any(|f| f.contains("pom.xml") || f.contains("build.gradle"))) {
        project_type = "Java";
        test_framework = "JUnit";
        test_directory = "src/test/java/";
    }
    // Check for Go project
    else if file_extensions.contains("go") || diff_info.commits.iter().any(|c| c.files_changed.iter().any(|f| f.contains("go.mod"))) {
        project_type = "Go";
        test_framework = "go test";
        test_directory = "same package as source files";
    }
    // Check for C/C++ project
    else if file_extensions.contains("c") || file_extensions.contains("cpp") || file_extensions.contains("h") || file_extensions.contains("hpp") {
        project_type = "C/C++";
        test_framework = "Google Test or Catch2";
        test_directory = "tests/";
    }
    // Check for C# project
    else if file_extensions.contains("cs") || diff_info.commits.iter().any(|c| c.files_changed.iter().any(|f| f.contains(".csproj") || f.contains(".sln"))) {
        project_type = "C#";
        test_framework = "NUnit or xUnit";
        test_directory = "Tests/";
    }
    
    ProjectInfo {
        project_type: project_type.to_string(),
        test_framework: test_framework.to_string(),
        test_directory: test_directory.to_string(),
    }
}

fn create_rust_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    // For Rust, create a single comprehensive test file that actually works
    let file_path = test_dir.join("integration_tests.rs");
    
    let content = format!(
        "// Integration tests for commit-buddy\n// Generated by commit-buddy\n\nuse commit_buddy::ai::*;\nuse commit_buddy::git::*;\nuse commit_buddy::config::*;\nuse anyhow::Result;\n\n{}\n\n// Additional helper tests\n#[tokio::test]\nasync fn test_config_loading() -> Result<()> {{\n    let config = Config::load()?;\n    assert_eq!(config.get_default_branch(), \"master\");\n    Ok(())\n}}\n\n#[tokio::test]\nasync fn test_ai_fallback_without_key() -> Result<()> {{\n    let config = Config {{\n        default_branch: \"master\".to_string(),\n        openai_api_key: None,\n        github_token: None,\n    }};\n    \n    let result = call_openai_api(\"test\", \"test\", &config).await?;\n    assert!(result.contains(\"🤖 AI Feature Unavailable\"));\n    Ok(())\n}}",
        test_content
    );
    
    fs::write(file_path, content)?;
    println!("📝 Created test file: tests/integration_tests.rs");
    Ok(())
}

fn create_python_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let test_files = vec![
        ("test_ai.py", "AI module tests"),
        ("test_git.py", "Git module tests"),
        ("test_config.py", "Config module tests"),
    ];
    
    for (filename, description) in test_files {
        let file_path = test_dir.join(filename);
        let content = format!("# {}\n# Generated by commit-buddy\n\n{}", description, test_content);
        fs::write(file_path, content)?;
        println!("📝 Created test file: tests/{}", filename);
    }
    Ok(())
}

fn create_js_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let test_files = vec![
        ("ai.test.js", "AI module tests"),
        ("git.test.js", "Git module tests"),
        ("config.test.js", "Config module tests"),
    ];
    
    for (filename, description) in test_files {
        let file_path = test_dir.join(filename);
        let content = format!("// {}\n// Generated by commit-buddy\n\n{}", description, test_content);
        fs::write(file_path, content)?;
        println!("📝 Created test file: tests/{}", filename);
    }
    Ok(())
}

fn create_java_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let test_files = vec![
        ("AiTests.java", "AI module tests"),
        ("GitTests.java", "Git module tests"),
        ("ConfigTests.java", "Config module tests"),
    ];
    
    for (filename, description) in test_files {
        let file_path = test_dir.join(filename);
        let content = format!("// {}\n// Generated by commit-buddy\n\n{}", description, test_content);
        fs::write(file_path, content)?;
        println!("📝 Created test file: tests/{}", filename);
    }
    Ok(())
}

fn create_go_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let test_files = vec![
        ("ai_test.go", "AI module tests"),
        ("git_test.go", "Git module tests"),
        ("config_test.go", "Config module tests"),
    ];
    
    for (filename, description) in test_files {
        let file_path = test_dir.join(filename);
        let content = format!("// {}\n// Generated by commit-buddy\n\n{}", description, test_content);
        fs::write(file_path, content)?;
        println!("📝 Created test file: tests/{}", filename);
    }
    Ok(())
}

fn create_cpp_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let test_files = vec![
        ("ai_tests.cpp", "AI module tests"),
        ("git_tests.cpp", "Git module tests"),
        ("config_tests.cpp", "Config module tests"),
    ];
    
    for (filename, description) in test_files {
        let file_path = test_dir.join(filename);
        let content = format!("// {}\n// Generated by commit-buddy\n\n{}", description, test_content);
        fs::write(file_path, content)?;
        println!("📝 Created test file: tests/{}", filename);
    }
    Ok(())
}

fn create_csharp_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let test_files = vec![
        ("AiTests.cs", "AI module tests"),
        ("GitTests.cs", "Git module tests"),
        ("ConfigTests.cs", "Config module tests"),
    ];
    
    for (filename, description) in test_files {
        let file_path = test_dir.join(filename);
        let content = format!("// {}\n// Generated by commit-buddy\n\n{}", description, test_content);
        fs::write(file_path, content)?;
        println!("📝 Created test file: tests/{}", filename);
    }
    Ok(())
}

fn create_generic_tests(test_content: &str, test_dir: &Path) -> Result<()> {
    let file_path = test_dir.join("tests.txt");
    let content = format!("// Generic tests\n// Generated by commit-buddy\n\n{}", test_content);
    fs::write(file_path, content)?;
    println!("📝 Created test file: tests/tests.txt");
    Ok(())
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