# Guidance discovery

Both standalone and desktop MCP servers expose the same bundled skills and rules. They are available before opening an artwork. The catalog is embedded at build time from `playbook/`, with no runtime shell, plugin or developer-checkout dependency. Release archives and installers also include the readable source bundle and its MIT license and notices.

After `initialize` and `notifications/initialized`, use:

```json
{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"skill://index"}}
{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"skill://studio"}}
{"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"rules://index"}}
{"jsonrpc":"2.0","id":6,"method":"resources/read","params":{"uri":"rules://00-core-principles"}}
```

Skills use `skill://<directory-name>`. Numbered rule Markdown files use `rules://<filename-without-extension>`. Supporting text files use their path relative to the skill directory, for example `skill://pixel-art/references/fundamentals.md` and `skill://pixen/examples/animated.json`. Lists are deterministic, complete single pages; no cursor is needed. URIs are exact and case-sensitive. Unknown URIs, paths, traversal, query strings and unlisted resources fail rather than reading external files.

Workflow entry points are also exposed through `prompts/list` and `prompts/get`:

```json
{"jsonrpc":"2.0","id":7,"method":"prompts/get","params":{"name":"pixen:draw"}}
```

Prompts take no arguments and return the workflow instructions; send the artwork request in the conversation. Foundation bundles (`pixen` and `pixel-art`) are resources, rather than workflow prompts. MCP prompts do not install `/pixen:draw` slash commands. A host may show MCP prompts in its own picker or support a separate skill mechanism. Pixen does not ship the upstream Claude/omp plugin or register the specialist descriptions as agents.

For clients/providers with tool access but no resource support:

```json
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"read_guidance","arguments":{"uri":"skill://draw"}}}
```

This read-only tool returns the same catalog text. Pixen's embedded AI advertises it through the existing app-only allowlist and receives guidance-navigation instructions with each turn. It cannot use it to read arbitrary files, access integrations or delegate. The application continues to own saving and file actions. Gallery submission is excluded from the catalog and shipped bundle.

Imported instructions describe procedures and some planned capabilities. Always inspect `tools/list` and its exact schemas. Discovery does not implement missing editing operations, and no legacy-shape guessing or shell fallback is authorized. Rebuild and reconnect after changing the source playbook; editing installed Markdown does not change the binary's catalog.

## Packaged-install smoke check

On Windows, build and package the actual tools, then run:

```sh
cargo build --locked --release --bin pixen-cli --bin pixen-mcp
bun scripts/package-tools.ts --target x86_64-pc-windows-msvc
bun scripts/smoke-guidance.ts --archive target/packages/Pixen-tools-0.2.0-x86_64-pc-windows-msvc.zip
```

The smoke check extracts the ZIP into a new temporary installation, launches its MCP executable from an unrelated empty working directory, loads `skill://draw` and one of its referenced rules, and compares MCP resource/tool/prompt output with the installed bundle. It cleans up its process and extracted files. It accepts application or tools ZIPs; tar.gz packages and real provider UI sessions require separate platform verification.
