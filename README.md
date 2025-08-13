# Postium Mail

<div align="center">
  <img src="https://img.shields.io/badge/Tauri-2.0-blue?style=for-the-badge" alt="Tauri">
  <img src="https://img.shields.io/badge/React-18.2-61DAFB?style=for-the-badge&logo=react" alt="React">
  <img src="https://img.shields.io/badge/TypeScript-5.8-3178C6?style=for-the-badge&logo=typescript" alt="TypeScript">
  <img src="https://img.shields.io/badge/License-MIT-green?style=for-the-badge" alt="License">
</div>

<div align="center">
  <h3>🚀 A modern, fast, and secure desktop email client built with Tauri and React</h3>
</div>

## ✨ Features

- 📧 **Three-Pane Layout** - Classic email client interface with folders, email list, and email preview
- 🌍 **Multi-language Support** - Full i18n support (English and Chinese)
- 🎨 **Modern UI** - Built with Microsoft Fluent UI components
- 🔄 **Real-time Sync** - Synchronize emails across multiple accounts
- 📝 **Rich Text Editor** - Compose emails with full formatting support
- 🔍 **Advanced Search** - Quick email search and filtering
- 🎯 **Smart Organization** - Labels, folders, and automatic categorization
- 🌓 **Dark/Light Theme** - Toggle between themes with one click
- 📊 **Resizable Panels** - Customize layout with draggable dividers
- 📁 **Multiple Accounts** - Manage multiple email accounts in one place
- 🔐 **Secure** - Built on Tauri's secure architecture
- 📱 **Cross-Platform** - Works on Windows, macOS, and Linux

## 📸 Screenshots

<div align="center">
  <img src="docs/images/screenshot-main.png" alt="Main Interface" width="800">
  <p><i>Main email interface with three-pane layout</i></p>
</div>

## 🛠️ Tech Stack

### Frontend
- **React 18.2** - UI framework
- **TypeScript** - Type safety
- **Fluent UI** - Microsoft's design system
- **Zustand** - State management
- **i18next** - Internationalization
- **react-window** - Virtual scrolling for performance
- **date-fns** - Date manipulation

### Backend
- **Tauri 2.0** - Desktop application framework
- **Rust** - Backend language
- **SQLite** - Local data storage (planned)

## 📦 Installation

### Prerequisites

- [Node.js](https://nodejs.org/) (v16 or higher)
- [Rust](https://www.rust-lang.org/) (latest stable)
- [Yarn](https://yarnpkg.com/) package manager

### Development Setup

1. **Clone the repository**
```bash
git clone https://github.com/yourusername/postium-mail.git
cd postium-mail
```

2. **Install dependencies**
```bash
yarn install
```

3. **Run in development mode**
```bash
yarn tauri dev
```

### Building for Production

```bash
# Build for current platform
yarn tauri build

# The built application will be in src-tauri/target/release
```

## 🚀 Quick Start

After launching Postium Mail:

1. **Add Email Account** - Click the account menu to add your email account
2. **Sync Emails** - Click the sync button to fetch your emails
3. **Navigate** - Use the folder pane to navigate between folders
4. **Read** - Click on any email to view its content
5. **Compose** - Click the "Compose" button to write a new email

## 🌐 Internationalization

Postium Mail currently supports:
- 🇬🇧 English (en)
- 🇨🇳 Chinese Simplified (zh)

To change language: Click the language icon in the toolbar and select your preferred language.

## 🎨 Customization

### Themes
Toggle between light and dark themes using the theme button in the toolbar.

### Layout
- Drag the dividers between panels to resize them
- Your layout preferences are automatically saved
- Toggle folder pane visibility with the navigation button

## 📝 Development

### Project Structure

```
postium-mail/
├── src/                    # React frontend source
│   ├── components/         # React components
│   ├── i18n/              # Internationalization files
│   ├── stores/            # Zustand stores
│   ├── types/             # TypeScript definitions
│   └── utils/             # Utility functions
├── src-tauri/             # Rust backend source
│   ├── src/               # Rust source files
│   └── icons/             # Application icons
├── package.json           # Node dependencies
└── tauri.conf.json        # Tauri configuration
```

### Available Scripts

```bash
# Development
yarn dev              # Start Vite dev server
yarn tauri dev        # Start Tauri in development mode

# Building
yarn build           # Build React app
yarn tauri build     # Build Tauri application

# Type Checking
yarn tsc             # Run TypeScript compiler

# Cleaning
yarn clean           # Clean build artifacts
```

### Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 🐛 Known Issues

- Email provider integration is currently using mock data
- Some advanced email features are still in development
- Custom context menu is implemented but not fully integrated

## 📋 Roadmap

- [ ] IMAP/SMTP Integration
- [ ] Calendar Integration
- [ ] Contact Management
- [ ] Email Templates
- [ ] Advanced Filtering Rules
- [ ] Email Encryption (PGP/GPG)
- [ ] Cloud Sync
- [ ] Mobile Companion App
- [ ] Plugin System
- [ ] More Language Support

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Tauri](https://tauri.app/) - For the amazing desktop framework
- [Microsoft Fluent UI](https://developer.microsoft.com/en-us/fluentui) - For the beautiful UI components
- [React](https://reactjs.org/) - For the powerful UI library
- All contributors who have helped shape this project

## 💬 Support

- **Documentation**: [Wiki](https://github.com/yourusername/postium-mail/wiki)
- **Issues**: [GitHub Issues](https://github.com/yourusername/postium-mail/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/postium-mail/discussions)

## 🌟 Star History

[![Star History Chart](https://api.star-history.com/svg?repos=yourusername/postium-mail&type=Date)](https://star-history.com/#yourusername/postium-mail&Date)

---

<div align="center">
  Made with ❤️ by the Postium Mail Team
</div>