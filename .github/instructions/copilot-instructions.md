# Tombola Game - AI Coding Assistant Instructions

- This is a Rust-based multi-binary tombola/bingo game with a client-server architecture. The main components include a server for managing the game state and two clients for interacting with the game.

## Behaviour Guidelines
- At first prompt consult `README.md` and `docs` directory.
- Always update `README.md` and `docs` when you make changes to the API or game logic.
- When changing documentation don't run tets.
- Do not build everytime unless you need to test changes in the code running the binaries.
- Be concise and clear in your responses.
- Unless you want to test the business logic do not build the binaries every time, just run `cargo check`.

## Code Rules
- Use `Context7 MCP` for language references, libraries, and tools and examples.
- For print statements, use this format: `println!("Hello, {NAME}. Nice to meet you!");`.
- Prefer references (Borrows '&') over clones or copies unless necessary.
- Use consistent types for variables, functions, and structs.
- Use IDE warnings to guide your code quality.
- Run `cargo clippy` when you achieve the objective to lint the code. If there are suggestions, apply to code with `cargo clippy --fix`. Allow `--dirty-code` if needed.

## Environment Info
- This is a Windows system with PowerShell, do not use bash commands.
- If you need to create test scripts, place them in the `tests/` directory.
