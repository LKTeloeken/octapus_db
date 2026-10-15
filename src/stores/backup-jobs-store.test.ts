import { beforeEach, describe, expect, it } from 'vitest';
import {
  JOB_LOG_LIMIT,
  useBackupJobsStore,
  type NewBackupJob,
} from './backup-jobs-store';

const store = () => useBackupJobsStore.getState();

const job = (jobId: string, label: string, path: string): NewBackupJob => ({
  jobId,
  kind: 'backup',
  serverId: 1,
  database: 'loja',
  label,
  path,
});

describe('backup-jobs-store', () => {
  beforeEach(() => useBackupJobsStore.setState({ jobs: {} }));

  it('acompanha um job do começo ao fim', () => {
    store().add(job('job-1', 'loja', '/tmp/loja.dump'));
    store().apply({ jobId: 'job-1', type: 'log', lines: ['a', 'b'] });
    store().apply({ jobId: 'job-1', type: 'log', lines: ['c'] });
    store().apply({
      jobId: 'job-1',
      type: 'done',
      path: '/tmp/loja.dump',
      sizeBytes: 42,
      elapsedMs: 1500,
      warnings: [],
    });

    expect(store().jobs['job-1']).toMatchObject({
      label: 'loja',
      status: 'done',
      log: ['a', 'b', 'c'],
      sizeBytes: 42,
      elapsedMs: 1500,
    });
  });

  it('aceita o evento que chega antes do add', () => {
    store().apply({ jobId: 'job-2', type: 'error', message: 'sem pg_dump' });
    store().add(job('job-2', 'loja › public', '/tmp/x.sql'));

    expect(store().jobs['job-2']).toMatchObject({
      label: 'loja › public',
      path: '/tmp/x.sql',
      status: 'error',
      error: 'sem pg_dump',
    });
  });

  it('guarda só o fim do log', () => {
    store().add(job('job-3', 'loja', '/tmp/x'));
    const lines = Array.from({ length: JOB_LOG_LIMIT + 10 }, (_, i) => `linha ${i}`);
    store().apply({ jobId: 'job-3', type: 'log', lines });

    const { log } = store().jobs['job-3'];
    expect(log).toHaveLength(JOB_LOG_LIMIT);
    expect(log[0]).toBe('linha 10');
  });

  it('guarda os avisos de uma restauração', () => {
    store().add({ ...job('job-5', 'loja', '/tmp/x.dump'), kind: 'restore' });
    store().apply({
      jobId: 'job-5',
      type: 'done',
      path: '',
      sizeBytes: 0,
      elapsedMs: 10,
      warnings: ['pg_restore: error: role "app" does not exist'],
    });
    expect(store().jobs['job-5']).toMatchObject({
      kind: 'restore',
      status: 'done',
      path: '/tmp/x.dump',
      warnings: ['pg_restore: error: role "app" does not exist'],
    });
  });

  it('fechar tira o job da lista', () => {
    store().add(job('job-4', 'loja', '/tmp/x'));
    store().apply({ jobId: 'job-4', type: 'cancelled' });
    expect(store().jobs['job-4'].status).toBe('cancelled');
    store().dismiss('job-4');
    expect(store().jobs['job-4']).toBeUndefined();
  });
});
