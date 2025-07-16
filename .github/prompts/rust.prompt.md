---
mode: ask
---
- This is a Rust-based multi-binary tombola/bingo game with a client-server architecture. The main components include a server for managing the game state and two clients for interacting with the game. The server handles the game logic, while the clients provide terminal interfaces for players.
- Do not build everytime unless you need to test changes in the code running the binaries.
- This is a Windows system with PowerShell.
- Under doc/you will find TOMBOLA_API.md that contains the API documentation for the game server, keep it aligned for future API changes.
- Keep README.md updated.
- If you need to create test scripts, place them in the `tests/` directory.
- Run cargo clippy when you achieve the objective to lint the code.