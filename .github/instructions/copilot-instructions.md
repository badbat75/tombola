# Tombola Game - AI Coding Assistant Instructions

- This is a Rust-based multi-binary tombola/bingo game with a client-server architecture. The main components include a server for managing the game state and two clients for interacting with the game.

## Behaviour Guidelines
- Do not build everytime unless you need to test changes in the code running the binaries.
- Be concise and clear in your responses.
- Unless you want to test the business logic do not build the binaries every time, just run `cargo check`.
- Update documentation only when you make changes to the code and tested.

## Documentation Guidelines
- Do not add additional considerations to TOMBOLA_API.md, FLOWS.md, and DATA.md keep it focused respectively on API, game flows, and data model.
- Use `mermaid` syntax for diagrams where needed.
- Documents should be in `docs/` directory.
- At first prompt consult `README.md` and `docs` directory.
- Always update `README.md` and `docs` when you make changes to the API or game logic.
- When changing documentation don't run tests.

## Code Rules
- Use `Context7 MCP`.
- For logging and print statements, use this format: `("Hello, {NAME}. Nice to meet you!")` instead of `("Hello, {}. Nice to meet you!", NAME)`.
- Prefer references (Borrows '&') over clones or copies unless necessary.
- Use consistent types for variables, functions, and structs.
- Use IDE warnings to guide your code quality.
- Run `cargo clippy` when you achieve the objective to lint the code. If there are suggestions, apply to code with `cargo clippy --fix`. Allow `--dirty-code` if needed.

## Environment Info
- This is a Windows system with PowerShell, do not use bash commands.
- If you need to create test scripts, place them in the `tests/` directory.
