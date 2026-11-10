# 02: Schema Parser and Types

**What to build:** Parser for `.env.schema` specification files. It reads schema files line-by-line, parsing entries, default values, descriptions, and `@required`, `@sensitive`, and `@type` decorators (`string`, `port`, `url`, `bool`, `integer`, `enum(...)`).

**Blocked by:** None (can start immediately)

**Status:** done

- [x] `SchemaEntry` and `SchemaType` defined in `src/schema/types.rs` matching SPEC §6.2.
- [x] `parse_schema_file` and `parse_schema_str` implemented in `src/schema/parser.rs` matching SPEC §7.
- [x] Correctly extracts `@required`, `@sensitive`, and type constraints (`@type=string`, `port`, `url`, `bool`, `integer`, `enum(a,b,c)`).
- [x] Attaches preceding comments as description and parses inline default values (`KEY=default`).
- [x] Unit tests for parsing various decorators, edge cases, and blank lines.
