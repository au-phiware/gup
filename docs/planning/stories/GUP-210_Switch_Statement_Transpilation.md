# GUP-210: Switch Statement Transpilation

## Story Overview

**Title**: Switch Statement Transpilation from Rust Match to WGSL  
**Epic**: Phase 2 Initiative 4 - Rust-to-WGSL Transpilation  
**Priority**: Low  
**Story Points**: 5  
**Status**: 🚧 In Progress

## Context

GUP-058 implemented control flow transpilation but explicitly excluded match
expression conversion, producing an error instead. WGSL supports `switch`
statements for integer matching, and simple Rust match expressions on integers
could be automatically converted.

## User Story

**As a** shader function developer  
**I want** to use simple Rust match expressions in shader functions  
**So that** I can write readable branching logic on integer values without
manually converting to if-else chains

## Problem Statement

Rust match expressions on integers map naturally to WGSL switch statements, but
complex patterns (guards, destructuring, references) have no WGSL equivalent. We
need to support the common case while producing clear errors for unsupported
patterns.

## Acceptance Criteria

- [ ] Convert `match x { 0 => ..., 1 => ..., _ => ... }` to WGSL switch
- [ ] Support integer literal patterns and default/wildcard arms
- [ ] Error with clear message on unsupported patterns (guards, ranges, etc.)
- [ ] Add `Switch` variant to `WgslStatement` AST
- [ ] Test suite covering supported and unsupported match patterns

## Dependencies

- GUP-058: Control Flow Handling (provides the converter architecture)

## Testing Strategy

- Unit tests for integer match → switch conversion
- Error tests for unsupported pattern types
- Pipeline tests for end-to-end transpilation

## Definition of Done

- [ ] Integer match expressions transpile to WGSL switch statements
- [ ] Clear error messages for unsupported patterns
- [ ] Test coverage for all supported cases
