# Contributing to MDM

Thank you for your interest in contributing to MDM (Markdown+Media)!

## Development Setup

### Prerequisites

- Node.js 16+
- Python 3.8+
- Rust 1.70+ (optional, for core development)

### Setup

```bash
# Clone the repository
git clone https://github.com/seunghan91/markdown-media.git
cd markdown-media

# Install JavaScript dependencies
cd packages/parser-js
npm install

# Install Python dependencies
cd ../parser-py
pip install -r requirements.txt

# Build Rust core (optional)
cd ../../core
cargo build
```

## Project Structure

```
markdown-media/
├── packages/
│   ├── parser-js/      # JavaScript parser
│   ├── parser-py/      # Python helpers
│   └── parser-rs/      # Rust parser (planned)
├── core/               # Rust core engine
├── converters/         # Document converters
├── cli/                # CLI tool
├── viewer/             # HTML viewer
└── docs/               # Documentation
```

## Development Workflow

1. **Create a branch**

   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make changes**

   - Write code
   - Add tests
   - Update documentation

3. **Test your changes**

   ```bash
   # JavaScript
   cd packages/parser-js
   npm test

   # Rust
   cd core
   cargo test

   # Python
   cd packages/parser-py
   pytest
   ```

4. **Commit**

   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

   Follow [Conventional Commits](https://www.conventionalcommits.org/):

   - `feat:` - New feature
   - `fix:` - Bug fix
   - `docs:` - Documentation
   - `refactor:` - Code refactoring
   - `test:` - Adding tests

5. **Push and create PR**
   ```bash
   git push origin feature/your-feature-name
   ```

## Code Style

### JavaScript

- Use ES6+ features
- 2 spaces for indentation
- Semicolons optional but consistent

### Python

- Follow PEP 8
- Use type hints where appropriate
- 4 spaces for indentation

### Rust

- Follow `rustfmt` conventions
- Run `cargo fmt` before committing

## Testing

- Write tests for all new features
- Maintain or improve code coverage
- Ensure all tests pass before submitting PR

## Documentation

- Update README.md if adding new features
- Add JSDoc comments for JavaScript functions
- Update relevant documentation in `docs/`

## Questions?

Open an issue or reach out to the maintainers.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
