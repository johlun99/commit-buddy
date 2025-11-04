# Commit Buddy 🤖

An AI-powered git companion for enhanced development workflow. Commit Buddy helps you write better commit messages, generate PR descriptions, create unit tests, and more!

## Demo
Below you can find a demo-video which displays the current functionallity

## Disclaimer ☢️
This is an unfinished product made during a hackathon. No time was spent on security, maintainability and so on. Proceed at own risk.

Known issues:
- Auto generating unit tests _works_ but openAI seems very fond of sending back tests with syntax errors making them unusable.
- Loading state not rendering correctly.
- Other stuff...

## Features

- **📝 PR Description Generation**: Automatically generate comprehensive PR descriptions from commit messages and code changes
- **🧪 Unit Test Generation**: Generate unit tests for your code changes
- **✨ Commit Message Improvement**: Get AI suggestions for better commit messages
- **📋 Changelog Generation**: Create professional changelogs from your commits
- **🔍 Code Review Assistance**: Get AI-powered code review suggestions
- **💬 Interactive Commit Assistant**: Get help writing commit messages interactively

## Installation

```bash
# Clone the repository
git clone <your-repo-url>
cd commit-buddy

# Build the project
cargo build --release

# Install globally (optional)
cargo install --path .
```

## Usage

### Generate PR Description
```bash
# Generate PR description comparing to master branch (default)
commit-buddy pr-description

# Compare to a specific branch
commit-buddy pr-description --base develop

# Output as JSON
commit-buddy pr-description --format json
```

### Generate Unit Tests
```bash
# Generate tests for changed code
commit-buddy generate-tests

# Specify test framework
commit-buddy generate-tests --framework pytest
```

### Improve Commit Messages
```bash
# Improve the last commit message
commit-buddy improve-commit

# Improve a specific commit
commit-buddy improve-commit --commit abc123
```

### Interactive Commit Assistant
```bash
# Get commit message suggestions for staged changes
commit-buddy commit

# Stage all changes and get suggestions
commit-buddy commit --all
```

### Generate Changelog
```bash
# Generate changelog from commits
commit-buddy changelog

# Save to file
commit-buddy changelog --output CHANGELOG.md
```

### Code Review
```bash
# Get AI code review suggestions
commit-buddy review
```

## Configuration

### Environment Variables

- `COMMIT_BUDDY_DEFAULT_BRANCH`: Default branch to compare against (default: master)
- `OPENAI_API_KEY`: Your OpenAI API key for AI features
- `GITHUB_TOKEN`: Your GitHub token for GitHub integration

### Example .env file
```env
COMMIT_BUDDY_DEFAULT_BRANCH=master
OPENAI_API_KEY=your_openai_api_key_here
GITHUB_TOKEN=your_github_token_here
```

## Development

### Prerequisites
- Rust 1.70+
- Git
- OpenAI API key (for AI features)

### Building
```bash
cargo build
```

### Running Tests
```bash
cargo test
```

### Running the CLI
```bash
cargo run -- <command> [options]
```

## Architecture

The project is structured as follows:

- `src/main.rs`: CLI entry point and command parsing
- `src/git.rs`: Git operations and repository analysis
- `src/ai.rs`: AI-powered features (currently template-based)
- `src/github.rs`: GitHub API integration
- `src/utils.rs`: Utility functions

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details

## Roadmap

- [ ] Full OpenAI API integration
- [ ] Interactive terminal UI (like lazygit)
- [ ] Git hook integration
- [ ] Configuration file support
- [ ] Plugin system
- [ ] More AI models support
- [ ] Performance optimizations

## AI Integration Status

The tool is now ready for AI integration! Here's the current status:

### ✅ **What's Working:**
- **Configuration System**: Environment variable support for API keys
- **Git Analysis**: Full commit and diff analysis
- **CLI Interface**: Complete command structure
- **Error Handling**: Graceful fallbacks when API key is missing

### 🔧 **Setting Up AI Features:**

1. **Get OpenAI API Key:**
   - Visit [OpenAI Platform](https://platform.openai.com/api-keys)
   - Create a new API key

2. **Configure Environment:**
   ```bash
   # Create .env file
   cp env.example .env
   
   # Edit .env and add your API key
   OPENAI_API_KEY=your_actual_api_key_here
   ```

3. **Test AI Features:**
   ```bash
   # This will now use real AI
   commit-buddy pr-description
   ```

### 🚀 **Current Behavior:**
- **Without API Key**: Shows helpful message with instructions
- **With API Key**: Makes real OpenAI API calls to GPT-4
- **All Git Features**: Fully functional regardless of AI status

## Hackathon Notes

This project demonstrates:
- Modern Rust CLI development with clap
- Git repository analysis with git2
- Structured command architecture
- Extensible AI integration points
- Professional documentation and user experience
- Environment-based configuration system
