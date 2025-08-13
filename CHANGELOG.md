# Changelog

All notable changes to Postium Mail will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- New features that have been added but not yet released

### Changed
- Changes in existing functionality

### Deprecated
- Features that will be removed in future versions

### Removed
- Features that have been removed

### Fixed
- Bug fixes

### Security
- Security vulnerability fixes

## [0.1.0] - 2024-01-XX

### Added
- ✨ Initial release of Postium Mail
- 📧 Three-pane email client layout (Folders, Email List, Email Detail)
- 🌍 Multi-language support (English and Chinese)
- 🎨 Modern UI built with Microsoft Fluent UI components
- 🌓 Dark and light theme support with one-click toggle
- 📊 Resizable panels with state persistence
- 📁 Multiple email account support
- 🔄 Email synchronization with mock data
- 📝 Rich text email composition
- 🔍 Email search and filtering
- 🏷️ Labels and folder organization
- ⭐ Star and flag email marking
- 📎 Attachment support (UI only)
- 🗂️ Archive and trash functionality
- 📱 Cross-platform support (Windows, macOS, Linux)
- 📊 Virtual scrolling for performance
- 🔐 Secure architecture based on Tauri
- 📝 Comprehensive logging system with rotation
- 🚫 Disabled default context menu with selective text selection
- 💾 Local storage for user preferences
- 🎯 Auto-select first email on load
- 🌐 Language auto-detection based on system settings
- ⚡ Fast and responsive UI with React 18
- 🛠️ TypeScript for type safety
- 📦 Modular component architecture
- 🎨 Consistent design system
- 📊 State management with Zustand
- 🔄 Automatic log cleanup (30-day retention)
- 📜 MIT License

### Technical Details
- Frontend: React 18.2.0 with TypeScript
- UI Framework: Microsoft Fluent UI v9
- Desktop Framework: Tauri 2.0
- State Management: Zustand
- Internationalization: i18next
- Build Tool: Vite
- Package Manager: Yarn
- Backend Language: Rust

### Known Issues
- Email provider integration using mock data only
- Custom context menu not fully integrated
- Some TypeScript warnings with react-window types

## [0.0.1] - 2024-01-01

### Added
- 🚀 Initial project setup
- 📁 Basic project structure
- ⚙️ Development environment configuration
- 📦 Dependencies installation
- 🏗️ Build system setup

---

## Version History

- **0.1.0** - Initial public release with core features
- **0.0.1** - Project initialization

## Upgrade Guide

### From 0.0.x to 0.1.0
1. Clean install recommended:
   ```bash
   rm -rf node_modules yarn.lock
   yarn install
   ```
2. Clear application data if upgrading from development version
3. Re-configure email accounts

## Contributors

Thank you to all contributors who have helped shape Postium Mail!

## Links

- [GitHub Repository](https://github.com/yourusername/postium-mail)
- [Issue Tracker](https://github.com/yourusername/postium-mail/issues)
- [Documentation](https://github.com/yourusername/postium-mail/wiki)
- [Releases](https://github.com/yourusername/postium-mail/releases)