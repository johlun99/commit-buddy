use anyhow::Result;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub default_branch: String,
    pub openai_api_key: Option<String>,
    pub github_token: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_branch: "master".to_string(),
            openai_api_key: None,
            github_token: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut config = Self::default();
        
        // Load from environment variables
        if let Ok(branch) = env::var("COMMIT_BUDDY_DEFAULT_BRANCH") {
            config.default_branch = branch;
        }
        
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            config.openai_api_key = Some(api_key);
        }
        
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            config.github_token = Some(token);
        }
        
        // Also check for GH_TOKEN as an alternative
        if config.github_token.is_none() {
            if let Ok(token) = env::var("GH_TOKEN") {
                config.github_token = Some(token);
            }
        }
        
        Ok(config)
    }
    
    pub fn get_default_branch(&self) -> &str {
        &self.default_branch
    }
    
    pub fn has_openai_key(&self) -> bool {
        self.openai_api_key.is_some()
    }
    
    pub fn has_github_token(&self) -> bool {
        self.github_token.is_some()
    }
}
