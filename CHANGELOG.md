# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-02-27

### Added
- **MCP Integration**: Introduced `McpManager` and `McpTool` to support the Model Context Protocol (MCP). Agents can now connect to external MCP servers (via Stdio or SSE) and use their tools seamlessly.
- **Tool Registry**: Implemented a `ToolRegistry` and `Tool` trait in `AgentActor`, allowing agents to register and execute local or remote tools.
- **Output Parser**: Added `OutputParser` trait and `JsonParser` implementation to extract tool calls from LLM responses.

### Changed
- **ActorSystem Optimization**: Refactored `hibernate` method to use polling with timeout instead of fixed sleep, eliminating race conditions during actor shutdown.
- **AgentActor Refactoring**: Massive refactoring of `handle_agent_message`. Extracted message handling logic into dedicated private methods (`handle_user_prompt`, `handle_tool_result`, etc.) for better maintainability and readability.
- **Dependency Updates**: Upgraded dependencies and added `mcp-core`, `mcp-client`, `url` for MCP support.

### Fixed
- Fixed potential race conditions in actor hibernation process.
