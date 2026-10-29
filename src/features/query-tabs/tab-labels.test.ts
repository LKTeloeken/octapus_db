import { describe, expect, it } from 'vitest';
import { tabDescriptions, tabLocation } from './tab-labels';

const SERVERS: Record<number, string> = { 1: 'prod', 2: 'staging' };
const name = (id: number) => SERVERS[id] ?? '';

const tab = (
  id: string,
  title: string,
  serverId: number,
  database: string,
  schema: string | null,
) => ({ id, title, serverId, database, schema });

describe('tabDescriptions', () => {
  it('não rotula abas de título único', () => {
    const result = tabDescriptions(
      [
        tab('a', 'users', 1, 'app', 'public'),
        tab('b', 'orders', 2, 'app', 'public'),
      ],
      name,
    );
    expect(result.size).toBe(0);
  });

  it('mesma tabela em schemas diferentes: só o schema', () => {
    const result = tabDescriptions(
      [
        tab('a', 'users', 1, 'saas', 'tenant_1'),
        tab('b', 'users', 1, 'saas', 'tenant_2'),
      ],
      name,
    );
    expect(result.get('a')).toBe('tenant_1');
    expect(result.get('b')).toBe('tenant_2');
  });

  it('mesma tabela em servidores diferentes: só o servidor', () => {
    const result = tabDescriptions(
      [
        tab('a', 'users', 1, 'app', 'public'),
        tab('b', 'users', 2, 'app', 'public'),
      ],
      name,
    );
    expect(result.get('a')).toBe('prod');
    expect(result.get('b')).toBe('staging');
  });

  it('vários níveis variando: todos eles, na ordem servidor · database · schema', () => {
    const result = tabDescriptions(
      [
        tab('a', 'users', 1, 'app', 'public'),
        tab('b', 'users', 2, 'app', 'audit'),
        tab('c', 'users', 2, 'legacy', 'public'),
      ],
      name,
    );
    expect(result.get('a')).toBe('prod · app · public');
    expect(result.get('b')).toBe('staging · app · audit');
    expect(result.get('c')).toBe('staging · legacy · public');
  });

  it('banco sem schema: não inventa rótulo vazio', () => {
    const result = tabDescriptions(
      [tab('a', 'users', 1, 'app', null), tab('b', 'users', 1, 'app', null)],
      name,
    );
    expect(result.size).toBe(0);
  });
});

describe('tabLocation', () => {
  it('omite o schema que não existe', () => {
    expect(tabLocation(tab('a', 'users', 1, 'app', null), 'prod')).toBe(
      'prod › app',
    );
    expect(tabLocation(tab('a', 'users', 1, 'app', 'public'), 'prod')).toBe(
      'prod › app › public',
    );
  });
});
