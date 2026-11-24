---
trigger: always_on
description: Use the codebase knowledge graph for structural code discovery, and check coverage before making claims.
---

# codebase-memory

For structural questions (who calls this, what does this call, where is this symbol, what depends on it), use the codebase-memory MCP tools before grep or file search. The project is `Users-fzrsahi-Documents-Coding-CodeIsland`.

- `search_graph` to find symbols by name.
- `trace_path` for callers and callees.
- `get_code_snippet` for exact source.
- `check_index_coverage` for every cited path and for the scope behind a negative or exhaustive claim.
- `query_graph` for multi-hop patterns.

Evidence tiers: a scout lookup is provisional and must not support negative claims. A verification pass should include both call directions and exact snippets. An audit should paginate fully and disclose every limitation.

After a graph result, call `check_index_coverage` once with all evidence paths. A clean result means no recorded gap, not proof of completeness. If the index is missing or stale for a path, read the source directly and say so. Use grep for string literals, config values, and non-code files.

The graph is indexed from `main`. Work in a PR branch may contain code the graph has not seen, so read the branch source for claims about its changes.
