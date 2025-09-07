# Dump Plugin (RS-32 Formatting)

The built-in `dump` plugin streams scan messages to stdout for inspection and debugging.

## Output Formats

| Format  | CLI Flag          | Description |
|---------|-------------------|-------------|
| text    | `--text` (default)| Human-friendly pretty summaries per message. |
| raw     | (config only)     | Legacy pre-RS-32 header + JSON body formatting (for regression). |
| compact | `--compact`       | Single-line terse summary per message. |
| json    | `--json`          | Full structured JSON object (one per line). |

### Selecting Formats

CLI flags take precedence over configuration. To request the legacy format explicitly via configuration:

```toml
[dump]
default_format = "raw"
```

If no flag is provided and no `default_format` is set, the pretty `text` format is used.

## Pretty Text Examples

```
#1@scanner-1 ScanStarted repo=/work/project branch=main filters:files=all authors=all max_commits=none date_range=all ts=2025-09-06T12:00:00Z
#2@scanner-1 Commit abcdef1 Alice <alice@example.com> +10/-2 parents=root ts=2025-09-06T12:00:01Z msg="Initial commit"
#3@scanner-1 FileChange Modified path=src/lib.rs +5/-1 bin=false commit=abcdef1 ts=2025-09-06T12:00:02Z
#10@scanner-1 ScanCompleted commits=120 files=34 +3456/-789 duration=60s ts=2025-09-06T12:01:00Z
```

## Raw Format

The `raw` format preserves legacy output for backward compatibility and golden tests. It is intentionally not exposed as a CLI flag to keep the surface area minimal.

## Compact Format

Designed for quick scanning and piping into further tools. One line per message, stable key ordering where possible.

## JSON Format

Each message is a complete JSON object containing header and message content. Suitable for downstream ingestion.

## Colour Output (Planned Enhancements)

Colour (ANSI) is auto-disabled when:
- Output is not a TTY
- `NO_COLOR` environment variable is present

Future enhancements may add explicit force flags or integration with global CLI colour controls. Until then, formatting avoids reliance on colour for critical information.

## Fallback Behaviour

The formatter exhaustively matches current `ScanMessage` variants. If new variants are added, a fallback path will surface a clear tag (to be implemented alongside new variants).

## Testing & Stability

Golden tests lock the legacy `raw` format. Pretty text tests cover all current message variants. Additional edge-case tests will cover rename/copy and multi-parent commits.

## Configuration Summary

| Key              | Location        | Effect |
|------------------|-----------------|--------|
| `default_format` | `[dump]` in TOML | Sets initial format if no CLI flag. Accepts `text`, `raw`, `compact`, `json`. |

## Rationale for `raw` vs `text`

`raw` exists solely to preserve regression stability while allowing the human-facing `text` format to evolve. Consumers should migrate to `text` unless byte-for-byte stability with pre-RS-32 output is required.

---

_Last updated: RS-32 implementation phase (pretty text + raw preservation)._
