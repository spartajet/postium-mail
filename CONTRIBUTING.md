# Contributing to Postium Mail

First off, thank you for considering contributing to Postium Mail! It's people like you that make Postium Mail such a great tool. We welcome contributions from everyone, regardless of their experience level.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How Can I Contribute?](#how-can-i-contribute)
- [Development Setup](#development-setup)
- [Style Guidelines](#style-guidelines)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Community](#community)

## Code of Conduct

By participating in this project, you are expected to uphold our Code of Conduct:

- **Be Respectful**: Value each other's ideas, styles, and viewpoints
- **Be Direct but Professional**: Honest and direct communication
- **Be Inclusive**: Welcome newcomers and encourage diverse perspectives
- **Be Understanding**: Disagreements happen, but frustration should not turn into personal attacks
- **Be Patient**: Remember that everyone was new to open source once

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally
3. Create a new branch for your contribution
4. Make your changes
5. Push to your fork and submit a pull request

## How Can I Contribute?

### Reporting Bugs

Before creating bug reports, please check existing issues to avoid duplicates. When creating a bug report, include:

- **Clear title and description**
- **Steps to reproduce**
- **Expected behavior**
- **Actual behavior**
- **Screenshots** (if applicable)
- **System information** (OS, version, etc.)
- **Error messages** and logs

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion, include:

- **Clear title and description**
- **Use case** - Explain why this enhancement would be useful
- **Possible implementation** - If you have ideas on how to implement it
- **Alternatives** - Any alternative solutions you've considered

### Your First Code Contribution

Unsure where to begin? Look for these labels:

- `good first issue` - Good for newcomers
- `help wanted` - Extra attention needed
- `documentation` - Documentation improvements
- `translation` - Help with internationalization

### Pull Requests

1. Include screenshots and animated GIFs in your pull request whenever possible
2. Follow the [style guidelines](#style-guidelines)
3. Include thoughtfully-worded, well-structured tests
4. Document new code
5. End all files with a newline

## Development Setup

### Prerequisites

- Node.js 16+
- Rust 1.70+
- Yarn package manager
- Git

### Setting Up Your Development Environment

```bash
# Clone your fork
git clone https://github.com/your-username/postium-mail.git
cd postium-mail

# Add upstream remote
git remote add upstream https://github.com/original-owner/postium-mail.git

# Install dependencies
yarn install

# Start development server
yarn tauri dev
```

### Running Tests

```bash
# Run frontend tests
yarn test

# Run Rust tests
cd src-tauri
cargo test

# Type checking
yarn tsc --noEmit
```

## Style Guidelines

### TypeScript/JavaScript Style Guide

- Use TypeScript for all new code
- Follow the existing code style
- Use meaningful variable and function names
- Add comments for complex logic
- Keep functions small and focused
- Use async/await over promises when possible

```typescript
// Good
async function fetchUserData(userId: string): Promise<User> {
  try {
    const response = await api.get(`/users/${userId}`);
    return response.data;
  } catch (error) {
    logger.error('Failed to fetch user data', error);
    throw error;
  }
}

// Bad
function fetchUserData(id) {
  return api.get('/users/' + id).then(r => r.data);
}
```

### React Component Guidelines

- Use functional components with hooks
- Keep components small and focused
- Use proper TypeScript types for props
- Memoize expensive computations
- Extract reusable logic into custom hooks

```tsx
// Good
interface EmailItemProps {
  email: Email;
  onClick: (email: Email) => void;
}

export const EmailItem: React.FC<EmailItemProps> = React.memo(({ email, onClick }) => {
  const handleClick = useCallback(() => {
    onClick(email);
  }, [email, onClick]);

  return (
    <div onClick={handleClick}>
      {/* component content */}
    </div>
  );
});
```

### Rust Style Guide

- Follow Rust naming conventions
- Use `rustfmt` to format code
- Add documentation comments for public APIs
- Handle errors properly with `Result`
- Write unit tests for new functionality

```rust
/// Processes an email and returns the parsed result
/// 
/// # Arguments
/// * `raw_email` - The raw email content to process
/// 
/// # Returns
/// * `Result<Email, Error>` - The parsed email or an error
pub fn process_email(raw_email: &str) -> Result<Email, Error> {
    // Implementation
}
```

### CSS/Styling Guidelines

- Use Fluent UI design tokens when possible
- Keep styles modular and scoped
- Use semantic class names
- Avoid inline styles except for dynamic values
- Support both light and dark themes

## Commit Guidelines

We use [Conventional Commits](https://www.conventionalcommits.org/) for clear commit messages.

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `perf`: Performance improvements
- `ci`: CI/CD changes
- `build`: Build system changes
- `revert`: Reverting previous commits

### Examples

```bash
# Feature
feat(email): add attachment preview support

# Bug fix
fix(ui): correct panel resize handle position

# Documentation
docs(readme): update installation instructions

# Style
style(components): format code with prettier

# Refactor
refactor(store): simplify email state management
```

## Pull Request Process

1. **Update Documentation** - Update README.md with details of changes if needed
2. **Add Tests** - Ensure your code has appropriate test coverage
3. **Follow Style Guide** - Ensure your code follows the project's style guidelines
4. **Update Types** - Update TypeScript types if you've changed interfaces
5. **Test Locally** - Ensure all tests pass and the app runs correctly
6. **Request Review** - Request review from maintainers
7. **Address Feedback** - Be responsive to feedback and make requested changes
8. **Squash Commits** - Squash commits if requested

### Pull Request Template

When creating a PR, include:

```markdown
## Description
Brief description of what this PR does

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] I have tested this locally
- [ ] I have added tests
- [ ] All tests pass

## Screenshots (if applicable)
Add screenshots here

## Checklist
- [ ] My code follows the style guidelines
- [ ] I have performed a self-review
- [ ] I have commented my code where necessary
- [ ] I have updated the documentation
- [ ] My changes generate no new warnings
```

## Project Structure

Understanding the project structure helps you know where to make changes:

```
postium-mail/
├── src/                    # Frontend source
│   ├── components/         # React components
│   │   └── [Component]/    # Component folder
│   │       ├── index.tsx   # Component implementation
│   │       └── styles.ts   # Component styles
│   ├── i18n/              # Internationalization
│   │   └── locales/       # Translation files
│   ├── stores/            # State management
│   ├── types/             # TypeScript types
│   └── utils/             # Utility functions
├── src-tauri/             # Backend source
│   ├── src/               # Rust source files
│   └── Cargo.toml         # Rust dependencies
├── docs/                  # Documentation
└── tests/                 # Test files
```

## Translation Guidelines

When adding new UI text:

1. Add the key to all locale files (`src/i18n/locales/*.json`)
2. Use meaningful, hierarchical keys
3. Include context in comments if needed
4. Test with all supported languages

```json
{
  "email": {
    "compose": {
      "title": "New Message",
      "send": "Send",
      "saveDraft": "Save Draft"
    }
  }
}
```

## Testing Guidelines

### Unit Tests
- Test individual functions and components
- Mock external dependencies
- Cover edge cases

### Integration Tests
- Test component interactions
- Test store updates
- Test API integrations

### E2E Tests
- Test complete user workflows
- Test cross-platform compatibility
- Test performance

## Community

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General discussions and questions
- **Discord**: Real-time chat (if available)
- **Email**: Contact maintainers for security issues

## Recognition

Contributors will be recognized in:
- The project README
- Release notes
- Special contributors file

## Questions?

Feel free to:
- Open an issue for questions
- Start a discussion
- Contact the maintainers

Thank you for contributing to Postium Mail! 🎉