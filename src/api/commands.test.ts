import { readdirSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { RustCommand } from './commands';

/**
 * As três listas de comandos andam juntas: o que o backend registra
 * (`generate_handler!` em src-tauri/src/lib.rs), o enum `RustCommand` e os
 * handlers do mock (`pnpm dev:mock`). Comando novo sem um dos três quebra aqui,
 * não na tela.
 */
const read = (path: string) =>
  readFileSync(new URL(path, import.meta.url), 'utf8');

function registeredInRust(): string[] {
  const lib = read('../../src-tauri/src/lib.rs');
  const start = lib.indexOf('generate_handler![');
  const block = lib.slice(start, lib.indexOf('])', start));
  return Array.from(block.matchAll(/commands::(\w+)/g), m => m[1]);
}

function handledByMock(): string[] {
  const dir = new URL('../mocks/', import.meta.url);
  const names = readdirSync(dir)
    .filter(file => file.endsWith('.ts') && !file.endsWith('.test.ts'))
    .flatMap(file =>
      Array.from(
        read(`../mocks/${file}`).matchAll(/\[RustCommand\.(\w+)\]\s*:/g),
        m => m[1],
      ),
    );
  return names.map(
    member => RustCommand[member as keyof typeof RustCommand] ?? member,
  );
}

const sorted = (list: string[]) => Array.from(new Set(list)).sort();

describe('comandos do backend', () => {
  const enumValues = sorted(Object.values(RustCommand));

  it('o enum RustCommand espelha o generate_handler! do lib.rs', () => {
    expect(enumValues).toEqual(sorted(registeredInRust()));
  });

  it('todo comando tem handler no mock', () => {
    const missing = enumValues.filter(
      command => !handledByMock().includes(command),
    );
    expect(missing).toEqual([]);
  });
});
